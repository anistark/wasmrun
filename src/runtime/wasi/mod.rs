//! WASI (WebAssembly System Interface) integration
//!
//! Registers memory-bridged host functions so the executor can dispatch
//! WASI imports through the linker.

pub mod network;
pub mod syscalls;

use crate::runtime::core::executor::WASI_PROC_EXIT_PREFIX;
use crate::runtime::core::linker::{ClosureHostFunction, Linker};
use crate::runtime::core::values::Value;
use crate::runtime::wasi::network::NetworkAccess;
use std::collections::{HashMap, VecDeque};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub const WASI_STDIN_FD: u32 = 0;
pub const WASI_STDOUT_FD: u32 = 1;
pub const WASI_STDERR_FD: u32 = 2;
pub const WASI_FIRST_PREOPEN_FD: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdKind {
    Stdin,
    Stdout,
    Stderr,
    PreopenDir,
    File,
    Directory,
    /// A listening socket handed to the guest by the host. WASI Preview 1 gives
    /// a guest no way to create one, so every listener is preopened the way a
    /// directory is.
    SocketListener,
    /// A connected socket, always the result of `sock_accept`.
    SocketStream,
}

/// The host side of a socket fd.
///
/// Kept beside `FdEntry` rather than inside it: that type is `Clone` and
/// describes a path, while these are live kernel objects shared by reference.
#[derive(Debug, Clone)]
pub enum SocketHandle {
    Listener(Arc<TcpListener>),
    Stream(Arc<TcpStream>),
    /// Created by `sock_open` and not yet connected.
    Unconnected,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FdEntry {
    pub kind: FdKind,
    pub host_path: PathBuf,
    pub guest_path: String,
    pub offset: u64,
    pub flags: u16,
}

pub struct WasiEnv {
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    /// Input handed to the program on fd 0. Empty means an immediate EOF,
    /// which is what a program sees when the caller supplied no stdin.
    stdin: Vec<u8>,
    /// How much of `stdin` has been consumed; reads resume from here so a
    /// program can drain it across several `fd_read` calls.
    stdin_pos: usize,
    fd_table: HashMap<u32, FdEntry>,
    next_fd: u32,
    #[allow(dead_code)]
    preopens: Vec<(String, PathBuf)>,
    /// Cap on combined stdout + stderr bytes captured. `None` = unlimited.
    max_output_bytes: Option<usize>,
    /// Set once captured output is dropped because the cap was reached.
    output_truncated: bool,
    /// Cap on the size of any single file written via WASI `fd_write`.
    /// `None` = unlimited. Enforced in the syscall layer.
    max_file_size: Option<u64>,
    /// Cap on the session's total on-disk usage (bytes). `None` = unlimited.
    /// Enforced in the syscall layer against the running `disk_used` counter.
    max_disk_bytes: Option<u64>,
    /// Running estimate of the session work-dir's total file bytes, maintained
    /// incrementally by the file syscalls (write adds, unlink/truncate subtract)
    /// so the cap can be checked in O(1) without walking the tree each write.
    /// Seeded from an actual directory scan at session start / before each exec.
    disk_used: u64,
    /// Live socket objects, keyed by the fd that names them in `fd_table`.
    sockets: HashMap<u32, SocketHandle>,
    /// What this execution may connect to. Nothing, unless configured.
    network: NetworkAccess,
    /// Connections accepted by a `poll_oneoff` readiness probe, waiting for
    /// the `sock_accept` that follows it. A listener cannot be asked whether a
    /// connection is waiting without taking it, so a poll that finds one keeps
    /// it here rather than dropping it on the floor.
    pending_accepts: HashMap<u32, VecDeque<TcpStream>>,
    /// The executor's cancellation flag, when one was installed. `poll_oneoff`
    /// watches it while it sleeps: the flag is how the agent's wall-clock
    /// timeout stops a running execution, and the executor can only check it
    /// between instructions, which is never while a host function is blocked.
    cancel: Option<Arc<AtomicBool>>,
}

impl WasiEnv {
    pub fn new() -> Self {
        let mut fd_table = HashMap::new();
        fd_table.insert(
            WASI_STDIN_FD,
            FdEntry {
                kind: FdKind::Stdin,
                host_path: PathBuf::new(),
                guest_path: String::new(),
                offset: 0,
                flags: 0,
            },
        );
        fd_table.insert(
            WASI_STDOUT_FD,
            FdEntry {
                kind: FdKind::Stdout,
                host_path: PathBuf::new(),
                guest_path: String::new(),
                offset: 0,
                flags: 0,
            },
        );
        fd_table.insert(
            WASI_STDERR_FD,
            FdEntry {
                kind: FdKind::Stderr,
                host_path: PathBuf::new(),
                guest_path: String::new(),
                offset: 0,
                flags: 0,
            },
        );

        WasiEnv {
            args: Vec::new(),
            env_vars: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdin: Vec::new(),
            stdin_pos: 0,
            fd_table,
            next_fd: WASI_FIRST_PREOPEN_FD,
            preopens: Vec::new(),
            sockets: HashMap::new(),
            network: NetworkAccess::denied(),
            pending_accepts: HashMap::new(),
            max_output_bytes: None,
            output_truncated: false,
            max_file_size: None,
            max_disk_bytes: None,
            disk_used: 0,
            cancel: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    #[allow(dead_code)] // TODO: Used by agent session builder
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.push((key, value));
        self
    }

    pub fn with_preopen(mut self, guest_path: &str, host_path: impl AsRef<Path>) -> Self {
        let host = host_path.as_ref().to_path_buf();
        let fd = self.next_fd;
        self.next_fd += 1;
        self.fd_table.insert(
            fd,
            FdEntry {
                kind: FdKind::PreopenDir,
                host_path: host.clone(),
                guest_path: guest_path.to_string(),
                offset: 0,
                flags: 0,
            },
        );
        self.preopens.push((guest_path.to_string(), host));
        self
    }

    /// Hand the guest a listening socket on the next free fd.
    ///
    /// Preview 1 has no call that creates a listener, so a server in the
    /// sandbox can only serve on one the host bound for it and passed in, the
    /// same shape as a preopened directory. The listener is put in
    /// non-blocking mode here: `sock_accept` waits in slices so it can watch
    /// the cancellation flag, which it could not do inside a blocking accept.
    pub fn with_tcp_listener(mut self, listener: TcpListener) -> Self {
        self.add_tcp_listener(listener);
        self
    }

    /// Hand the guest a listening socket and report the fd it landed on.
    ///
    /// The builder form above is what `wasmrun exec` uses, where the fd is
    /// predictable because nothing else is preopened first. A caller that has
    /// to *tell* the guest which fd to accept on needs the number back: a
    /// session preopens its work directory first, so the listener is not fd 3
    /// there and a hardcoded guess would be wrong.
    pub fn add_tcp_listener(&mut self, listener: TcpListener) -> u32 {
        let _ = listener.set_nonblocking(true);
        let fd = self.next_fd;
        self.next_fd += 1;
        let name = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "socket".to_string());
        self.fd_table.insert(
            fd,
            FdEntry {
                kind: FdKind::SocketListener,
                host_path: PathBuf::new(),
                guest_path: name,
                offset: 0,
                flags: 0,
            },
        );
        self.sockets
            .insert(fd, SocketHandle::Listener(Arc::new(listener)));
        fd
    }

    /// Give this execution a network. Absent this call it has none.
    pub fn set_network(&mut self, network: NetworkAccess) {
        self.network = network;
    }

    pub fn network(&self) -> &NetworkAccess {
        &self.network
    }

    /// Allocate an unconnected socket, the fd `sock_connect` later fills in.
    pub fn add_unconnected_socket(&mut self) -> u32 {
        let fd = self.allocate_fd(FdEntry {
            kind: FdKind::SocketStream,
            host_path: PathBuf::new(),
            guest_path: "socket".to_string(),
            offset: 0,
            flags: 0,
        });
        self.sockets.insert(fd, SocketHandle::Unconnected);
        fd
    }

    /// Attach a connected stream to an fd `sock_open` already handed out.
    pub fn connect_socket(&mut self, fd: u32, stream: TcpStream) {
        if let Some(entry) = self.fd_table.get_mut(&fd) {
            entry.guest_path = stream
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| "socket".to_string());
        }
        self.sockets
            .insert(fd, SocketHandle::Stream(Arc::new(stream)));
    }

    /// Register an accepted connection and return the fd naming it.
    pub fn add_socket_stream(&mut self, stream: TcpStream) -> u32 {
        let name = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "socket".to_string());
        let fd = self.allocate_fd(FdEntry {
            kind: FdKind::SocketStream,
            host_path: PathBuf::new(),
            guest_path: name,
            offset: 0,
            flags: 0,
        });
        self.sockets
            .insert(fd, SocketHandle::Stream(Arc::new(stream)));
        fd
    }

    /// The socket behind an fd, if that fd names one.
    pub fn socket(&self, fd: u32) -> Option<SocketHandle> {
        self.sockets.get(&fd).cloned()
    }

    /// Park a connection a readiness probe had to take in order to see it.
    pub fn push_pending_accept(&mut self, listener_fd: u32, stream: TcpStream) {
        self.pending_accepts
            .entry(listener_fd)
            .or_default()
            .push_back(stream);
    }

    /// Take a parked connection, if a probe left one.
    pub fn take_pending_accept(&mut self, listener_fd: u32) -> Option<TcpStream> {
        self.pending_accepts
            .get_mut(&listener_fd)
            .and_then(|queue| queue.pop_front())
    }

    pub fn has_pending_accept(&self, listener_fd: u32) -> bool {
        self.pending_accepts
            .get(&listener_fd)
            .is_some_and(|queue| !queue.is_empty())
    }

    pub fn set_args(&mut self, args: Vec<String>) {
        self.args = args;
    }

    /// Set the bytes the program reads from fd 0, rewinding to the start.
    pub fn set_stdin(&mut self, stdin: Vec<u8>) {
        self.stdin = stdin;
        self.stdin_pos = 0;
    }

    /// Consume up to `max` bytes of stdin. An empty return means EOF, which is
    /// how a program learns the input is finished rather than blocking.
    pub fn read_stdin(&mut self, max: usize) -> Vec<u8> {
        let end = self.stdin.len().min(self.stdin_pos.saturating_add(max));
        let chunk = self.stdin[self.stdin_pos.min(self.stdin.len())..end].to_vec();
        self.stdin_pos = end;
        chunk
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn env_vars(&self) -> &[(String, String)] {
        &self.env_vars
    }

    pub fn get_stdout(&self) -> Vec<u8> {
        self.stdout.clone()
    }

    pub fn get_stderr(&self) -> Vec<u8> {
        self.stderr.clone()
    }

    /// Configure the combined stdout + stderr capture cap (`None` = unlimited).
    pub fn set_max_output_bytes(&mut self, max: Option<usize>) {
        self.max_output_bytes = max;
    }

    /// Configure the per-file write size cap (`None` = unlimited).
    pub fn set_max_file_size(&mut self, max: Option<u64>) {
        self.max_file_size = max;
    }

    /// The per-file write size cap, if any.
    pub fn max_file_size(&self) -> Option<u64> {
        self.max_file_size
    }

    /// Configure the total-disk-usage cap (`None` = unlimited).
    /// Install the executor's cancellation flag so a blocking syscall can
    /// give up when the execution is cancelled.
    pub fn set_cancel_token(&mut self, token: Option<Arc<AtomicBool>>) {
        self.cancel = token;
    }

    pub fn cancel_token(&self) -> Option<&Arc<AtomicBool>> {
        self.cancel.as_ref()
    }

    pub fn set_max_disk_bytes(&mut self, max: Option<u64>) {
        self.max_disk_bytes = max;
    }

    /// The total-disk-usage cap, if any.
    pub fn max_disk_bytes(&self) -> Option<u64> {
        self.max_disk_bytes
    }

    /// Reset the running disk-usage counter to a measured value (e.g. from an
    /// actual directory scan at session start or before an exec).
    pub fn seed_disk_used(&mut self, bytes: u64) {
        self.disk_used = bytes;
    }

    /// Current running disk-usage estimate (bytes).
    pub fn disk_used(&self) -> u64 {
        self.disk_used
    }

    /// Record `bytes` of growth against the running disk-usage counter.
    pub fn add_disk_used(&mut self, bytes: u64) {
        self.disk_used = self.disk_used.saturating_add(bytes);
    }

    /// Release `bytes` from the running disk-usage counter (file shrink/unlink).
    pub fn sub_disk_used(&mut self, bytes: u64) {
        self.disk_used = self.disk_used.saturating_sub(bytes);
    }

    /// Whether captured output was dropped because the output cap was reached.
    pub fn output_truncated(&self) -> bool {
        self.output_truncated
    }

    /// Append `bytes` to the stdout buffer, honoring the output cap.
    pub fn write_stdout(&mut self, bytes: &[u8]) {
        self.append_capped(true, bytes);
    }

    /// Append `bytes` to the stderr buffer, honoring the output cap.
    pub fn write_stderr(&mut self, bytes: &[u8]) {
        self.append_capped(false, bytes);
    }

    /// Append to stdout or stderr, never letting the combined buffers exceed
    /// `max_output_bytes`. Excess is dropped and `output_truncated` is set.
    fn append_capped(&mut self, to_stdout: bool, bytes: &[u8]) {
        let slice = match self.max_output_bytes {
            Some(max) => {
                let used = self.stdout.len() + self.stderr.len();
                if used >= max {
                    self.output_truncated = true;
                    return;
                }
                let room = max - used;
                if bytes.len() > room {
                    self.output_truncated = true;
                    &bytes[..room]
                } else {
                    bytes
                }
            }
            None => bytes,
        };
        if to_stdout {
            self.stdout.extend_from_slice(slice);
        } else {
            self.stderr.extend_from_slice(slice);
        }
    }

    /// Add an environment variable (appends to existing list).
    pub fn add_env(&mut self, key: String, value: String) {
        // Update existing or append
        if let Some(entry) = self.env_vars.iter_mut().find(|(k, _)| k == &key) {
            entry.1 = value;
        } else {
            self.env_vars.push((key, value));
        }
    }

    /// Clear captured stdout buffer.
    pub fn clear_stdout(&mut self) {
        self.stdout.clear();
        self.output_truncated = false;
    }

    /// Clear captured stderr buffer.
    pub fn clear_stderr(&mut self) {
        self.stderr.clear();
        self.output_truncated = false;
    }

    /// How many bytes of the supplied stdin have not been read yet.
    pub fn stdin_remaining(&self) -> usize {
        self.stdin.len().saturating_sub(self.stdin_pos)
    }

    pub fn get_fd(&self, fd: u32) -> Option<&FdEntry> {
        self.fd_table.get(&fd)
    }

    pub fn get_fd_mut(&mut self, fd: u32) -> Option<&mut FdEntry> {
        self.fd_table.get_mut(&fd)
    }

    pub fn close_fd(&mut self, fd: u32) -> bool {
        // Don't close stdio
        if fd <= WASI_STDERR_FD {
            return true;
        }
        // Dropping the handle is what closes the underlying socket, so it has
        // to go with the table entry rather than outliving it. A listener that
        // closes takes its parked connections with it.
        self.sockets.remove(&fd);
        self.pending_accepts.remove(&fd);
        self.fd_table.remove(&fd).is_some()
    }

    pub fn allocate_fd(&mut self, entry: FdEntry) -> u32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        self.fd_table.insert(fd, entry);
        fd
    }

    /// Resolve a guest path relative to a directory fd to a host path.
    pub fn resolve_path(&self, dir_fd: u32, path: &str) -> Result<PathBuf, String> {
        let dir_entry = self
            .fd_table
            .get(&dir_fd)
            .ok_or_else(|| format!("Bad fd: {dir_fd}"))?;

        match dir_entry.kind {
            FdKind::PreopenDir | FdKind::Directory => {}
            _ => return Err(format!("fd {dir_fd} is not a directory")),
        }

        let resolved = dir_entry.host_path.join(path);

        // Prevent path traversal
        if let (Ok(canon_base), Ok(canon_resolved)) = (
            std::fs::canonicalize(&dir_entry.host_path),
            if resolved.exists() {
                std::fs::canonicalize(&resolved)
            } else if let Some(parent) = resolved.parent() {
                std::fs::canonicalize(parent)
                    .map(|p| p.join(resolved.file_name().unwrap_or_default()))
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no parent",
                ))
            },
        ) {
            if !canon_resolved.starts_with(&canon_base) {
                return Err("Path escapes preopen directory".to_string());
            }
        }

        Ok(resolved)
    }

    #[allow(dead_code)]
    pub fn preopens(&self) -> &[(String, PathBuf)] {
        &self.preopens
    }
}

impl Default for WasiEnv {
    fn default() -> Self {
        Self::new()
    }
}

const WASI_MODULE: &str = "wasi_snapshot_preview1";

pub fn create_wasi_linker(env: Arc<Mutex<WasiEnv>>) -> Linker {
    let mut linker = Linker::new();

    // fd_write
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_write",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let iovs_ptr = i32_arg(&args, 1)? as u32;
                    let iovs_len = i32_arg(&args, 2)? as u32;
                    let nwritten_ptr = i32_arg(&args, 3)? as u32;
                    let errno = syscalls::fd_write(fd, iovs_ptr, iovs_len, nwritten_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // fd_read
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_read",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let iovs_ptr = i32_arg(&args, 1)? as u32;
                    let iovs_len = i32_arg(&args, 2)? as u32;
                    let nread_ptr = i32_arg(&args, 3)? as u32;
                    let errno = syscalls::fd_read(fd, iovs_ptr, iovs_len, nread_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // fd_close
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_close",
            Box::new(ClosureHostFunction::new(
                move |args, _mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let errno = syscalls::fd_close(fd, &env);
                    Ok(vec![Value::I32(errno)])
                },
                1,
                1,
            )),
        );
    }

    // fd_seek
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_seek",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let offset = i64_arg(&args, 1)?;
                    let whence = i32_arg(&args, 2)? as u32;
                    let newoffset_ptr = i32_arg(&args, 3)? as u32;
                    let errno = syscalls::fd_seek(fd, offset, whence, newoffset_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // fd_fdstat_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_fdstat_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let stat_ptr = i32_arg(&args, 1)? as u32;
                    Ok(vec![Value::I32(syscalls::fd_fdstat_get(
                        fd, stat_ptr, mem, &env,
                    ))])
                },
                2,
                1,
            )),
        );
    }

    // fd_prestat_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_prestat_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let buf_ptr = i32_arg(&args, 1)? as u32;
                    Ok(vec![Value::I32(syscalls::fd_prestat_get(
                        fd, buf_ptr, mem, &env,
                    ))])
                },
                2,
                1,
            )),
        );
    }

    // fd_prestat_dir_name
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_prestat_dir_name",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let buf_ptr = i32_arg(&args, 1)? as u32;
                    let buf_len = i32_arg(&args, 2)? as u32;
                    Ok(vec![Value::I32(syscalls::fd_prestat_dir_name(
                        fd, buf_ptr, buf_len, mem, &env,
                    ))])
                },
                3,
                1,
            )),
        );
    }

    // path_open
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_open",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let _dirflags = i32_arg(&args, 1)? as u32;
                    let path_ptr = i32_arg(&args, 2)? as u32;
                    let path_len = i32_arg(&args, 3)? as u32;
                    let oflags = i32_arg(&args, 4)? as u32;
                    let _fs_rights_base = i64_arg(&args, 5)?;
                    let _fs_rights_inheriting = i64_arg(&args, 6)?;
                    let fdflags = i32_arg(&args, 7)? as u32;
                    let fd_out_ptr = i32_arg(&args, 8)? as u32;
                    let errno = syscalls::path_open(
                        dir_fd, path_ptr, path_len, oflags, fdflags, fd_out_ptr, mem, &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                9,
                1,
            )),
        );
    }

    // path_filestat_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_filestat_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let _flags = i32_arg(&args, 1)? as u32;
                    let path_ptr = i32_arg(&args, 2)? as u32;
                    let path_len = i32_arg(&args, 3)? as u32;
                    let buf_ptr = i32_arg(&args, 4)? as u32;
                    let errno =
                        syscalls::path_filestat_get(dir_fd, path_ptr, path_len, buf_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                5,
                1,
            )),
        );
    }

    // path_create_directory
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_create_directory",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let path_ptr = i32_arg(&args, 1)? as u32;
                    let path_len = i32_arg(&args, 2)? as u32;
                    let errno =
                        syscalls::path_create_directory(dir_fd, path_ptr, path_len, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                3,
                1,
            )),
        );
    }

    // path_unlink_file
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_unlink_file",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let path_ptr = i32_arg(&args, 1)? as u32;
                    let path_len = i32_arg(&args, 2)? as u32;
                    let errno = syscalls::path_unlink_file(dir_fd, path_ptr, path_len, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                3,
                1,
            )),
        );
    }

    // path_rename
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_rename",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let old_fd = i32_arg(&args, 0)? as u32;
                    let old_path_ptr = i32_arg(&args, 1)? as u32;
                    let old_path_len = i32_arg(&args, 2)? as u32;
                    let new_fd = i32_arg(&args, 3)? as u32;
                    let new_path_ptr = i32_arg(&args, 4)? as u32;
                    let new_path_len = i32_arg(&args, 5)? as u32;
                    let errno = syscalls::path_rename(
                        old_fd,
                        old_path_ptr,
                        old_path_len,
                        new_fd,
                        new_path_ptr,
                        new_path_len,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                6,
                1,
            )),
        );
    }

    // path_remove_directory
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_remove_directory",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let path_ptr = i32_arg(&args, 1)? as u32;
                    let path_len = i32_arg(&args, 2)? as u32;
                    let errno =
                        syscalls::path_remove_directory(dir_fd, path_ptr, path_len, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                3,
                1,
            )),
        );
    }

    // fd_readdir
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_readdir",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let buf_ptr = i32_arg(&args, 1)? as u32;
                    let buf_len = i32_arg(&args, 2)? as u32;
                    let cookie = i64_arg(&args, 3)?;
                    let bufused_ptr = i32_arg(&args, 4)? as u32;
                    let errno = syscalls::fd_readdir(
                        fd,
                        buf_ptr,
                        buf_len,
                        cookie as u64,
                        bufused_ptr,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                5,
                1,
            )),
        );
    }

    // fd_filestat_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "fd_filestat_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let buf_ptr = i32_arg(&args, 1)? as u32;
                    let errno = syscalls::fd_filestat_get(fd, buf_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                2,
                1,
            )),
        );
    }

    // args_sizes_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "args_sizes_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let count_ptr = i32_arg(&args, 0)? as u32;
                    let buf_size_ptr = i32_arg(&args, 1)? as u32;
                    let errno = syscalls::args_sizes_get(count_ptr, buf_size_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                2,
                1,
            )),
        );
    }

    // args_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "args_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let argv_ptr = i32_arg(&args, 0)? as u32;
                    let argv_buf_ptr = i32_arg(&args, 1)? as u32;
                    let errno = syscalls::args_get(argv_ptr, argv_buf_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                2,
                1,
            )),
        );
    }

    // environ_sizes_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "environ_sizes_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let count_ptr = i32_arg(&args, 0)? as u32;
                    let buf_size_ptr = i32_arg(&args, 1)? as u32;
                    let errno = syscalls::environ_sizes_get(count_ptr, buf_size_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                2,
                1,
            )),
        );
    }

    // environ_get
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "environ_get",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let environ_ptr = i32_arg(&args, 0)? as u32;
                    let environ_buf_ptr = i32_arg(&args, 1)? as u32;
                    let errno = syscalls::environ_get(environ_ptr, environ_buf_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                2,
                1,
            )),
        );
    }

    // clock_time_get
    linker.register(
        WASI_MODULE,
        "clock_time_get",
        Box::new(ClosureHostFunction::new(
            |args, mem| {
                let clock_id = i32_arg(&args, 0)? as u32;
                let precision = i64_arg(&args, 1)?;
                let time_ptr = i32_arg(&args, 2)? as u32;
                let errno = syscalls::clock_time_get(clock_id, precision, time_ptr, mem);
                Ok(vec![Value::I32(errno)])
            },
            3,
            1,
        )),
    );

    // random_get
    linker.register(
        WASI_MODULE,
        "random_get",
        Box::new(ClosureHostFunction::new(
            |args, mem| {
                let buf_ptr = i32_arg(&args, 0)? as u32;
                let buf_len = i32_arg(&args, 1)? as u32;
                Ok(vec![Value::I32(syscalls::random_get(
                    buf_ptr, buf_len, mem,
                ))])
            },
            2,
            1,
        )),
    );

    // proc_exit
    linker.register(
        WASI_MODULE,
        "proc_exit",
        Box::new(ClosureHostFunction::new(
            |args, _mem| {
                let code = i32_arg(&args, 0)?;
                Err(format!("{WASI_PROC_EXIT_PREFIX}{code}"))
            },
            1,
            0,
        )),
    );

    // fd_fdstat_set_flags
    linker.register(
        WASI_MODULE,
        "fd_fdstat_set_flags",
        Box::new(ClosureHostFunction::new(
            |args, _mem| {
                let fd = i32_arg(&args, 0)? as u32;
                let flags = i32_arg(&args, 1)? as u16;
                Ok(vec![Value::I32(syscalls::fd_fdstat_set_flags(fd, flags))])
            },
            2,
            1,
        )),
    );

    // path_filestat_set_times
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_filestat_set_times",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    // fd, lookupflags, path_ptr, path_len, atim, mtim, fst_flags.
                    // This was registered as taking six arguments while the
                    // signature has seven, so a guest that called it would have
                    // left the operand stack one value short.
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let _flags = i32_arg(&args, 1)? as u32;
                    let path_ptr = i32_arg(&args, 2)? as u32;
                    let path_len = i32_arg(&args, 3)? as u32;
                    let atim = i64_arg(&args, 4)? as u64;
                    let mtim = i64_arg(&args, 5)? as u64;
                    let fst_flags = i32_arg(&args, 6)? as u16;
                    let errno = syscalls::path_filestat_set_times(
                        dir_fd, path_ptr, path_len, atim, mtim, fst_flags, mem, &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                7,
                1,
            )),
        );
    }

    // path_readlink
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "path_readlink",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    // fd, path_ptr, path_len, buf_ptr, buf_len, buf_used_ptr.
                    // There is no lookupflags argument here, so the old
                    // registration read the buffer-used pointer one slot past
                    // the end of a seven-argument list that never existed.
                    let dir_fd = i32_arg(&args, 0)? as u32;
                    let path_ptr = i32_arg(&args, 1)? as u32;
                    let path_len = i32_arg(&args, 2)? as u32;
                    let buf_ptr = i32_arg(&args, 3)? as u32;
                    let buf_len = i32_arg(&args, 4)? as u32;
                    let buf_used_ptr = i32_arg(&args, 5)? as u32;
                    let errno = syscalls::path_readlink(
                        dir_fd,
                        path_ptr,
                        path_len,
                        buf_ptr,
                        buf_len,
                        buf_used_ptr,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                6,
                1,
            )),
        );
    }

    // sock_open (wasmrun extension: Preview 1 cannot create a socket)
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_open",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let family = i32_arg(&args, 0)? as u32;
                    let sock_type = i32_arg(&args, 1)? as u32;
                    let protocol = i32_arg(&args, 2)? as u32;
                    let fd_out_ptr = i32_arg(&args, 3)? as u32;
                    let errno =
                        syscalls::sock_open(family, sock_type, protocol, fd_out_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // sock_connect (wasmrun extension; takes a host string, not a sockaddr)
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_connect",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let addr_ptr = i32_arg(&args, 1)? as u32;
                    let addr_len = i32_arg(&args, 2)? as u32;
                    let port = i32_arg(&args, 3)? as u32;
                    let errno = syscalls::sock_connect(fd, addr_ptr, addr_len, port, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // sock_accept
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_accept",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let flags = i32_arg(&args, 1)? as u32;
                    let result_fd_ptr = i32_arg(&args, 2)? as u32;
                    let errno = syscalls::sock_accept(fd, flags, result_fd_ptr, mem, &env);
                    Ok(vec![Value::I32(errno)])
                },
                3,
                1,
            )),
        );
    }

    // sock_recv
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_recv",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let ri_data_ptr = i32_arg(&args, 1)? as u32;
                    let ri_data_len = i32_arg(&args, 2)? as u32;
                    let ri_flags = i32_arg(&args, 3)? as u32;
                    let ro_datalen_ptr = i32_arg(&args, 4)? as u32;
                    let ro_flags_ptr = i32_arg(&args, 5)? as u32;
                    let errno = syscalls::sock_recv(
                        fd,
                        ri_data_ptr,
                        ri_data_len,
                        ri_flags,
                        ro_datalen_ptr,
                        ro_flags_ptr,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                6,
                1,
            )),
        );
    }

    // sock_send
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_send",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let si_data_ptr = i32_arg(&args, 1)? as u32;
                    let si_data_len = i32_arg(&args, 2)? as u32;
                    let si_flags = i32_arg(&args, 3)? as u32;
                    let so_datalen_ptr = i32_arg(&args, 4)? as u32;
                    let errno = syscalls::sock_send(
                        fd,
                        si_data_ptr,
                        si_data_len,
                        si_flags,
                        so_datalen_ptr,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                5,
                1,
            )),
        );
    }

    // sock_shutdown
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "sock_shutdown",
            Box::new(ClosureHostFunction::new(
                move |args, _mem| {
                    let fd = i32_arg(&args, 0)? as u32;
                    let how = i32_arg(&args, 1)? as u32;
                    Ok(vec![Value::I32(syscalls::sock_shutdown(fd, how, &env))])
                },
                2,
                1,
            )),
        );
    }

    // path_symlink (stub — return ENOSYS)
    linker.register(
        WASI_MODULE,
        "path_symlink",
        Box::new(ClosureHostFunction::new(
            |_args, _mem| Ok(vec![Value::I32(syscalls::path_symlink())]),
            5,
            1,
        )),
    );

    // poll_oneoff
    {
        let env = env.clone();
        linker.register(
            WASI_MODULE,
            "poll_oneoff",
            Box::new(ClosureHostFunction::new(
                move |args, mem| {
                    let in_ptr = i32_arg(&args, 0)? as u32;
                    let out_ptr = i32_arg(&args, 1)? as u32;
                    let nsubscriptions = i32_arg(&args, 2)? as u32;
                    let nevents_ptr = i32_arg(&args, 3)? as u32;
                    let errno = syscalls::poll_oneoff(
                        in_ptr,
                        out_ptr,
                        nsubscriptions,
                        nevents_ptr,
                        mem,
                        &env,
                    );
                    Ok(vec![Value::I32(errno)])
                },
                4,
                1,
            )),
        );
    }

    // sched_yield
    linker.register(
        WASI_MODULE,
        "sched_yield",
        Box::new(ClosureHostFunction::new(
            |_args, _mem| Ok(vec![Value::I32(syscalls::sched_yield())]),
            0,
            1,
        )),
    );

    linker
}

fn i32_arg(args: &[Value], idx: usize) -> Result<i32, String> {
    match args.get(idx) {
        Some(Value::I32(v)) => Ok(*v),
        Some(other) => Err(format!("Expected i32 at arg {idx}, got {other:?}")),
        None => Err(format!("Missing arg {idx}")),
    }
}

fn i64_arg(args: &[Value], idx: usize) -> Result<i64, String> {
    match args.get(idx) {
        Some(Value::I64(v)) => Ok(*v),
        Some(Value::I32(v)) => Ok(*v as i64),
        Some(other) => Err(format!("Expected i64 at arg {idx}, got {other:?}")),
        None => Err(format!("Missing arg {idx}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::core::memory::LinearMemory;

    #[test]
    fn test_wasi_linker_has_all_syscalls() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env);

        let expected = [
            "fd_write",
            "fd_read",
            "fd_close",
            "fd_seek",
            "fd_fdstat_get",
            "fd_prestat_get",
            "fd_prestat_dir_name",
            "fd_readdir",
            "fd_filestat_get",
            "path_open",
            "path_filestat_get",
            "path_create_directory",
            "path_unlink_file",
            "path_rename",
            "path_remove_directory",
            "args_sizes_get",
            "args_get",
            "environ_sizes_get",
            "environ_get",
            "clock_time_get",
            "random_get",
            "proc_exit",
            "poll_oneoff",
            "sched_yield",
        ];

        for name in &expected {
            assert!(
                linker.has_import(WASI_MODULE, name),
                "Missing syscall: {name}"
            );
        }
    }

    #[test]
    fn test_fd_write_via_linker() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env.clone());
        let mut mem = LinearMemory::new(1, None).unwrap();

        mem.write_bytes(100, b"Hi").unwrap();
        mem.write_i32(0, 100).unwrap();
        mem.write_i32(4, 2).unwrap();

        let host_fn = linker.get_import(WASI_MODULE, "fd_write").unwrap();
        let result = host_fn
            .call(
                vec![Value::I32(1), Value::I32(0), Value::I32(1), Value::I32(16)],
                &mut mem,
            )
            .unwrap();

        assert_eq!(result[0], Value::I32(0));
        assert_eq!(mem.read_i32(16).unwrap(), 2);
        assert_eq!(env.lock().unwrap().get_stdout(), b"Hi");
    }

    #[test]
    fn test_proc_exit_via_linker() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let host_fn = linker.get_import(WASI_MODULE, "proc_exit").unwrap();
        let result = host_fn.call(vec![Value::I32(42)], &mut mem);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.starts_with(WASI_PROC_EXIT_PREFIX));
        let code: i32 = err
            .strip_prefix(WASI_PROC_EXIT_PREFIX)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(code, 42);
    }

    #[test]
    fn test_proc_exit_zero_via_linker() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let host_fn = linker.get_import(WASI_MODULE, "proc_exit").unwrap();
        let result = host_fn.call(vec![Value::I32(0)], &mut mem);
        assert!(result.is_err());
        let code: i32 = result
            .unwrap_err()
            .strip_prefix(WASI_PROC_EXIT_PREFIX)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn test_proc_exit_nonzero_via_linker() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let host_fn = linker.get_import(WASI_MODULE, "proc_exit").unwrap();
        let result = host_fn.call(vec![Value::I32(1)], &mut mem);
        assert!(result.is_err());
        let code: i32 = result
            .unwrap_err()
            .strip_prefix(WASI_PROC_EXIT_PREFIX)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn test_wasi_env_builder() {
        let env = WasiEnv::new()
            .with_args(vec!["prog".into(), "a".into()])
            .with_env("K".into(), "V".into());

        assert_eq!(env.args(), &["prog", "a"]);
        assert_eq!(env.env_vars(), &[("K".into(), "V".into())]);
        assert!(env.get_stdout().is_empty());
        assert!(env.get_stderr().is_empty());
    }

    #[test]
    fn test_wasi_env_preopen() {
        let tmp = std::env::temp_dir();
        let env = WasiEnv::new().with_preopen("/sandbox", &tmp);
        assert_eq!(env.preopens().len(), 1);
        assert_eq!(env.preopens()[0].0, "/sandbox");
        let fd_entry = env.get_fd(WASI_FIRST_PREOPEN_FD).unwrap();
        assert_eq!(fd_entry.kind, FdKind::PreopenDir);
        assert_eq!(fd_entry.guest_path, "/sandbox");
    }

    #[test]
    fn test_disk_used_counter_saturates() {
        let mut env = WasiEnv::new();
        assert_eq!(env.disk_used(), 0);
        env.seed_disk_used(5);
        env.add_disk_used(10);
        assert_eq!(env.disk_used(), 15);
        // Subtracting past zero floors at zero rather than underflowing.
        env.sub_disk_used(100);
        assert_eq!(env.disk_used(), 0);
    }

    #[test]
    fn test_output_cap_truncates_stdout() {
        let mut env = WasiEnv::new();
        env.set_max_output_bytes(Some(5));
        env.write_stdout(b"abc");
        assert!(!env.output_truncated());
        env.write_stdout(b"defgh"); // only "de" fits (5 total)
        assert_eq!(env.get_stdout(), b"abcde");
        assert!(env.output_truncated());
    }

    #[test]
    fn test_output_cap_is_combined_across_streams() {
        let mut env = WasiEnv::new();
        env.set_max_output_bytes(Some(4));
        env.write_stdout(b"ab");
        env.write_stderr(b"cdef"); // only "cd" fits
        assert_eq!(env.get_stdout(), b"ab");
        assert_eq!(env.get_stderr(), b"cd");
        assert!(env.output_truncated());
    }

    #[test]
    fn test_output_cap_unlimited_by_default() {
        let mut env = WasiEnv::new();
        env.write_stdout(&vec![b'x'; 100_000]);
        assert_eq!(env.get_stdout().len(), 100_000);
        assert!(!env.output_truncated());
    }

    #[test]
    fn test_clear_resets_truncation_flag() {
        let mut env = WasiEnv::new();
        env.set_max_output_bytes(Some(2));
        env.write_stdout(b"toolong");
        assert!(env.output_truncated());
        env.clear_stdout();
        assert!(!env.output_truncated());
    }

    #[test]
    fn test_prestat_get_via_linker() {
        let tmp = std::env::temp_dir();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", &tmp)));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let host_fn = linker.get_import(WASI_MODULE, "fd_prestat_get").unwrap();
        // fd 3 should be the preopen
        let result = host_fn
            .call(vec![Value::I32(3), Value::I32(0)], &mut mem)
            .unwrap();
        assert_eq!(result[0], Value::I32(0)); // ESUCCESS

        // prestat struct: u8 tag (0=dir) at offset 0, u32 name_len at offset 4
        assert_eq!(mem.read_u8(0).unwrap(), 0); // __WASI_PREOPENTYPE_DIR
        let name_len = mem.read_i32(4).unwrap();
        assert_eq!(name_len, 1); // "/" is 1 byte

        // fd 4 should return EBADF
        let result = host_fn
            .call(vec![Value::I32(4), Value::I32(0)], &mut mem)
            .unwrap();
        assert_eq!(result[0], Value::I32(syscalls::WASI_EBADF));
    }

    #[test]
    fn test_prestat_dir_name_via_linker() {
        let tmp = std::env::temp_dir();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/sandbox", &tmp)));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let host_fn = linker
            .get_import(WASI_MODULE, "fd_prestat_dir_name")
            .unwrap();
        let result = host_fn
            .call(
                vec![Value::I32(3), Value::I32(100), Value::I32(8)],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        let name = mem.read_bytes(100, 8).unwrap();
        assert_eq!(&name, b"/sandbox");
    }

    #[test]
    fn test_path_open_and_read_via_linker() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("hello.txt"), b"Hello FS!").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env.clone());
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Write path "hello.txt" at offset 200
        mem.write_bytes(200, b"hello.txt").unwrap();

        let path_open_fn = linker.get_import(WASI_MODULE, "path_open").unwrap();
        let result = path_open_fn
            .call(
                vec![
                    Value::I32(3),   // dir_fd
                    Value::I32(0),   // dirflags
                    Value::I32(200), // path_ptr
                    Value::I32(9),   // path_len
                    Value::I32(0),   // oflags
                    Value::I32(0),   // fs_rights_base (i64 as i32)
                    Value::I32(0),   // fs_rights_inheriting
                    Value::I32(0),   // fdflags
                    Value::I32(300), // fd_out_ptr
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        let opened_fd = mem.read_i32(300).unwrap();
        assert!(opened_fd >= 4);

        // Now fd_read from that fd
        // Set up iovec at offset 0: buf_ptr=400, buf_len=100
        mem.write_i32(0, 400).unwrap();
        mem.write_i32(4, 100).unwrap();

        let fd_read_fn = linker.get_import(WASI_MODULE, "fd_read").unwrap();
        let result = fd_read_fn
            .call(
                vec![
                    Value::I32(opened_fd),
                    Value::I32(0),   // iovs
                    Value::I32(1),   // iovs_len
                    Value::I32(500), // nread_ptr
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        let nread = mem.read_i32(500).unwrap();
        assert_eq!(nread, 9);

        let data = mem.read_bytes(400, 9).unwrap();
        assert_eq!(&data, b"Hello FS!");
    }

    #[test]
    fn test_path_create_directory_and_unlink_via_linker() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Create directory "subdir"
        mem.write_bytes(100, b"subdir").unwrap();
        let mkdir_fn = linker
            .get_import(WASI_MODULE, "path_create_directory")
            .unwrap();
        let result = mkdir_fn
            .call(
                vec![Value::I32(3), Value::I32(100), Value::I32(6)],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));
        assert!(tmp.path().join("subdir").is_dir());

        // Write a file in subdir: open with O_CREAT
        mem.write_bytes(200, b"subdir/test.txt").unwrap();
        let path_open_fn = linker.get_import(WASI_MODULE, "path_open").unwrap();
        let result = path_open_fn
            .call(
                vec![
                    Value::I32(3),
                    Value::I32(0),
                    Value::I32(200),
                    Value::I32(15),
                    Value::I32(1), // O_CREAT
                    Value::I32(0),
                    Value::I32(0),
                    Value::I32(0),
                    Value::I32(300),
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));
        assert!(tmp.path().join("subdir/test.txt").exists());

        // Unlink the file
        mem.write_bytes(400, b"subdir/test.txt").unwrap();
        let unlink_fn = linker.get_import(WASI_MODULE, "path_unlink_file").unwrap();
        let result = unlink_fn
            .call(
                vec![Value::I32(3), Value::I32(400), Value::I32(15)],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));
        assert!(!tmp.path().join("subdir/test.txt").exists());
    }

    #[test]
    fn test_fd_readdir_via_linker() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"aaa").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"bb").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        // fd 3 is the preopen dir, we can readdir on it directly
        let readdir_fn = linker.get_import(WASI_MODULE, "fd_readdir").unwrap();
        let result = readdir_fn
            .call(
                vec![
                    Value::I32(3),    // fd
                    Value::I32(0),    // buf_ptr
                    Value::I32(4096), // buf_len
                    Value::I32(0),    // cookie (i64)
                    Value::I32(8000), // bufused_ptr
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        let bufused = mem.read_i32(8000).unwrap();
        assert!(bufused > 0);
    }

    #[test]
    fn test_path_filestat_get_via_linker() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("data.bin"), b"12345").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        mem.write_bytes(100, b"data.bin").unwrap();
        let filestat_fn = linker.get_import(WASI_MODULE, "path_filestat_get").unwrap();
        let result = filestat_fn
            .call(
                vec![
                    Value::I32(3),
                    Value::I32(0),
                    Value::I32(100),
                    Value::I32(8),
                    Value::I32(200),
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        // filestat: size is at offset 32 (u64)
        let size = mem.read_i64(200 + 32).unwrap();
        assert_eq!(size, 5);

        // filetype at offset 16 (u8) should be REGULAR_FILE (4)
        let filetype = mem.read_u8(200 + 16).unwrap();
        assert_eq!(filetype, syscalls::WASI_FILETYPE_REGULAR_FILE);
    }

    #[test]
    fn test_path_rename_via_linker() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("old.txt"), b"content").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        mem.write_bytes(100, b"old.txt").unwrap();
        mem.write_bytes(200, b"new.txt").unwrap();

        let rename_fn = linker.get_import(WASI_MODULE, "path_rename").unwrap();
        let result = rename_fn
            .call(
                vec![
                    Value::I32(3),
                    Value::I32(100),
                    Value::I32(7),
                    Value::I32(3),
                    Value::I32(200),
                    Value::I32(7),
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));

        assert!(!tmp.path().join("old.txt").exists());
        assert!(tmp.path().join("new.txt").exists());
        assert_eq!(
            std::fs::read(tmp.path().join("new.txt")).unwrap(),
            b"content"
        );
    }

    #[test]
    fn test_fd_write_to_file_via_linker() {
        let tmp = tempfile::tempdir().unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let linker = create_wasi_linker(env);
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Open with O_CREAT
        mem.write_bytes(200, b"output.txt").unwrap();
        let path_open_fn = linker.get_import(WASI_MODULE, "path_open").unwrap();
        let result = path_open_fn
            .call(
                vec![
                    Value::I32(3),
                    Value::I32(0),
                    Value::I32(200),
                    Value::I32(10),
                    Value::I32(1), // O_CREAT
                    Value::I32(0),
                    Value::I32(0),
                    Value::I32(0),
                    Value::I32(300),
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));
        let fd = mem.read_i32(300).unwrap();

        // Write "hello" via fd_write
        mem.write_bytes(400, b"hello").unwrap();
        mem.write_i32(0, 400).unwrap();
        mem.write_i32(4, 5).unwrap();

        let fd_write_fn = linker.get_import(WASI_MODULE, "fd_write").unwrap();
        let result = fd_write_fn
            .call(
                vec![
                    Value::I32(fd),
                    Value::I32(0),
                    Value::I32(1),
                    Value::I32(500),
                ],
                &mut mem,
            )
            .unwrap();
        assert_eq!(result[0], Value::I32(0));
        assert_eq!(mem.read_i32(500).unwrap(), 5);

        // Verify on disk
        assert_eq!(
            std::fs::read(tmp.path().join("output.txt")).unwrap(),
            b"hello"
        );
    }
}
