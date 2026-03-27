//! WASI (WebAssembly System Interface) integration
//!
//! Registers memory-bridged host functions so the executor can dispatch
//! WASI imports through the linker.

pub mod syscalls;

use crate::runtime::core::executor::WASI_PROC_EXIT_PREFIX;
use crate::runtime::core::linker::{ClosureHostFunction, Linker};
use crate::runtime::core::values::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    fd_table: HashMap<u32, FdEntry>,
    next_fd: u32,
    #[allow(dead_code)]
    preopens: Vec<(String, PathBuf)>,
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
            fd_table,
            next_fd: WASI_FIRST_PREOPEN_FD,
            preopens: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    #[allow(dead_code)]
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.push((key, value));
        self
    }

    #[allow(dead_code)]
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

    pub fn stdout_mut(&mut self) -> &mut Vec<u8> {
        &mut self.stdout
    }

    pub fn stderr_mut(&mut self) -> &mut Vec<u8> {
        &mut self.stderr
    }

    /// Add an environment variable (appends to existing list).
    #[allow(dead_code)] // TODO: Used by agent session management (0.18.1)
    pub fn add_env(&mut self, key: String, value: String) {
        // Update existing or append
        if let Some(entry) = self.env_vars.iter_mut().find(|(k, _)| k == &key) {
            entry.1 = value;
        } else {
            self.env_vars.push((key, value));
        }
    }

    /// Clear captured stdout buffer.
    #[allow(dead_code)] // TODO: Used by agent session management (0.18.1)
    pub fn clear_stdout(&mut self) {
        self.stdout.clear();
    }

    /// Clear captured stderr buffer.
    #[allow(dead_code)] // TODO: Used by agent session management (0.18.1)
    pub fn clear_stderr(&mut self) {
        self.stderr.clear();
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

    // poll_oneoff (stub)
    linker.register(
        WASI_MODULE,
        "poll_oneoff",
        Box::new(ClosureHostFunction::new(
            |_args, _mem| Ok(vec![Value::I32(syscalls::WASI_ENOSYS)]),
            4,
            1,
        )),
    );

    // sched_yield (stub)
    linker.register(
        WASI_MODULE,
        "sched_yield",
        Box::new(ClosureHostFunction::new(
            |_args, _mem| Ok(vec![Value::I32(syscalls::WASI_ESUCCESS)]),
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
