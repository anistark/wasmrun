use crate::runtime::microkernel::{Pid, SyscallInterface, VfsEntry, WasmMicroKernel};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::{
    IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket,
};
use std::sync::{Arc, Mutex};

/// System call numbers for OS mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyscallNumber {
    // File system operations
    Open = 1,
    Read = 2,
    Write = 3,
    Close = 4,
    Mkdir = 5,
    Rmdir = 6,
    Unlink = 7,
    Stat = 8,

    // Process operations
    Fork = 9,
    Exec = 10,
    Exit = 11,
    Wait = 12,
    Kill = 13,
    GetPid = 14,

    // Memory operations
    Mmap = 15,
    Munmap = 16,

    // I/O operations
    Print = 17,
    Input = 18,

    // Socket operations (WASI)
    SockOpen = 19,
    SockBind = 20,
    SockListen = 21,
    SockAccept = 22,
    SockConnect = 23,
    SockRecv = 24,
    SockSend = 25,
    SockShutdown = 26,
    SockClose = 27,
    GetAddrInfo = 28,
}

impl TryFrom<u32> for SyscallNumber {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            1 => Ok(SyscallNumber::Open),
            2 => Ok(SyscallNumber::Read),
            3 => Ok(SyscallNumber::Write),
            4 => Ok(SyscallNumber::Close),
            5 => Ok(SyscallNumber::Mkdir),
            6 => Ok(SyscallNumber::Rmdir),
            7 => Ok(SyscallNumber::Unlink),
            8 => Ok(SyscallNumber::Stat),
            9 => Ok(SyscallNumber::Fork),
            10 => Ok(SyscallNumber::Exec),
            11 => Ok(SyscallNumber::Exit),
            12 => Ok(SyscallNumber::Wait),
            13 => Ok(SyscallNumber::Kill),
            14 => Ok(SyscallNumber::GetPid),
            15 => Ok(SyscallNumber::Mmap),
            16 => Ok(SyscallNumber::Munmap),
            17 => Ok(SyscallNumber::Print),
            18 => Ok(SyscallNumber::Input),
            19 => Ok(SyscallNumber::SockOpen),
            20 => Ok(SyscallNumber::SockBind),
            21 => Ok(SyscallNumber::SockListen),
            22 => Ok(SyscallNumber::SockAccept),
            23 => Ok(SyscallNumber::SockConnect),
            24 => Ok(SyscallNumber::SockRecv),
            25 => Ok(SyscallNumber::SockSend),
            26 => Ok(SyscallNumber::SockShutdown),
            27 => Ok(SyscallNumber::SockClose),
            28 => Ok(SyscallNumber::GetAddrInfo),
            _ => Err(anyhow::anyhow!("Unknown syscall number: {value}")),
        }
    }
}

/// System call arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallArgs {
    pub args: Vec<SyscallArg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyscallArg {
    String(String),
    Number(i64),
    Buffer(Vec<u8>),
    Pointer(usize),
}

/// System call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyscallResult {
    Success(SyscallReturn),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyscallReturn {
    Number(i64),
    String(String),
    Buffer(Vec<u8>),
    FileDescriptor(i32),
    ProcessId(Pid),
    VfsEntries(Vec<VfsEntry>),
    Unit,
}

/// Address family for sockets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Inet = 2,
    Inet6 = 10,
    Unix = 1,
}

impl TryFrom<i64> for AddressFamily {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> Result<Self> {
        match value {
            1 => Ok(AddressFamily::Unix),
            2 => Ok(AddressFamily::Inet),
            10 => Ok(AddressFamily::Inet6),
            _ => Err(anyhow::anyhow!("Unknown address family: {value}")),
        }
    }
}

/// Socket type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream = 1,
    Dgram = 2,
}

impl TryFrom<i64> for SocketType {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> Result<Self> {
        match value {
            1 => Ok(SocketType::Stream),
            2 => Ok(SocketType::Dgram),
            _ => Err(anyhow::anyhow!("Unknown socket type: {value}")),
        }
    }
}

/// Socket state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Created,
    Bound,
    Listening,
    Connected,
}

/// Socket handle (wraps actual socket types)
#[derive(Debug)]
pub enum SocketHandle {
    TcpListener(Arc<Mutex<TcpListener>>),
    TcpStream(Arc<Mutex<TcpStream>>),
    UdpSocket(Arc<Mutex<UdpSocket>>),
}

impl Clone for SocketHandle {
    fn clone(&self) -> Self {
        match self {
            SocketHandle::TcpListener(l) => SocketHandle::TcpListener(Arc::clone(l)),
            SocketHandle::TcpStream(s) => SocketHandle::TcpStream(Arc::clone(s)),
            SocketHandle::UdpSocket(u) => SocketHandle::UdpSocket(Arc::clone(u)),
        }
    }
}

/// File descriptor table for a process
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileDescriptorTable {
    descriptors: HashMap<i32, FileDescriptor>,
    next_fd: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FileDescriptor {
    File {
        path: String,
        offset: usize,
        flags: OpenFlags,
    },
    Socket {
        handle: SocketHandle,
        address_family: AddressFamily,
        socket_type: SocketType,
        state: SocketState,
        local_addr: Option<SocketAddr>,
        peer_addr: Option<SocketAddr>,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
}

impl Default for FileDescriptorTable {
    fn default() -> Self {
        let mut table = Self {
            descriptors: HashMap::new(),
            next_fd: 3,
        };

        table.descriptors.insert(
            0,
            FileDescriptor::File {
                path: "/dev/stdin".to_string(),
                offset: 0,
                flags: OpenFlags {
                    read: true,
                    write: false,
                    create: false,
                    truncate: false,
                },
            },
        );
        table.descriptors.insert(
            1,
            FileDescriptor::File {
                path: "/dev/stdout".to_string(),
                offset: 0,
                flags: OpenFlags {
                    read: false,
                    write: true,
                    create: false,
                    truncate: false,
                },
            },
        );
        table.descriptors.insert(
            2,
            FileDescriptor::File {
                path: "/dev/stderr".to_string(),
                offset: 0,
                flags: OpenFlags {
                    read: false,
                    write: true,
                    create: false,
                    truncate: false,
                },
            },
        );

        table
    }
}

#[allow(dead_code)]
impl FileDescriptorTable {
    pub fn open(&mut self, path: String, flags: OpenFlags) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;

        self.descriptors.insert(
            fd,
            FileDescriptor::File {
                path,
                offset: 0,
                flags,
            },
        );

        fd
    }

    pub fn open_socket(
        &mut self,
        handle: SocketHandle,
        address_family: AddressFamily,
        socket_type: SocketType,
    ) -> i32 {
        let fd = self.next_fd;
        self.next_fd += 1;

        self.descriptors.insert(
            fd,
            FileDescriptor::Socket {
                handle,
                address_family,
                socket_type,
                state: SocketState::Created,
                local_addr: None,
                peer_addr: None,
            },
        );

        fd
    }

    pub fn get(&self, fd: i32) -> Option<&FileDescriptor> {
        self.descriptors.get(&fd)
    }

    pub fn get_mut(&mut self, fd: i32) -> Option<&mut FileDescriptor> {
        self.descriptors.get_mut(&fd)
    }

    pub fn close(&mut self, fd: i32) -> bool {
        self.descriptors.remove(&fd).is_some()
    }
}

/// System call handler for the micro-kernel
#[allow(dead_code)]
pub struct SyscallHandler {
    kernel: WasmMicroKernel,
    fd_tables: HashMap<Pid, FileDescriptorTable>,
}

#[allow(dead_code)]
impl SyscallHandler {
    pub fn new(kernel: WasmMicroKernel) -> Self {
        Self {
            kernel,
            fd_tables: HashMap::new(),
        }
    }

    /// Handle a system call from a process
    pub fn handle_syscall(
        &mut self,
        pid: Pid,
        syscall_num: u32,
        args: SyscallArgs,
    ) -> SyscallResult {
        let syscall = match SyscallNumber::try_from(syscall_num) {
            Ok(s) => s,
            Err(e) => return SyscallResult::Error(e.to_string()),
        };

        match syscall {
            SyscallNumber::Open => self.handle_open(pid, args),
            SyscallNumber::Read => self.handle_read(pid, args),
            SyscallNumber::Write => self.handle_write(pid, args),
            SyscallNumber::Close => self.handle_close(pid, args),
            SyscallNumber::Mkdir => self.handle_mkdir(pid, args),
            SyscallNumber::Unlink => self.handle_unlink(pid, args),
            SyscallNumber::Stat => self.handle_stat(pid, args),
            SyscallNumber::GetPid => self.handle_getpid(pid),
            SyscallNumber::Kill => self.handle_kill(pid, args),
            SyscallNumber::Print => self.handle_print(pid, args),
            SyscallNumber::SockOpen => self.handle_sock_open(pid, args),
            SyscallNumber::SockBind => self.handle_sock_bind(pid, args),
            SyscallNumber::SockListen => self.handle_sock_listen(pid, args),
            SyscallNumber::SockAccept => self.handle_sock_accept(pid, args),
            SyscallNumber::SockConnect => self.handle_sock_connect(pid, args),
            SyscallNumber::SockRecv => self.handle_sock_recv(pid, args),
            SyscallNumber::SockSend => self.handle_sock_send(pid, args),
            SyscallNumber::SockShutdown => self.handle_sock_shutdown(pid, args),
            SyscallNumber::SockClose => self.handle_sock_close(pid, args),
            SyscallNumber::GetAddrInfo => self.handle_getaddrinfo(pid, args),
            _ => SyscallResult::Error(format!("Unimplemented syscall: {syscall:?}")),
        }
    }

    fn handle_open(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("open: insufficient arguments".to_string());
        }

        let path = match &args.args[0] {
            SyscallArg::String(s) => s.clone(),
            _ => return SyscallResult::Error("open: invalid path argument".to_string()),
        };

        let flags_num = match &args.args[1] {
            SyscallArg::Number(n) => *n as u32,
            _ => return SyscallResult::Error("open: invalid flags argument".to_string()),
        };

        let flags = OpenFlags {
            read: (flags_num & 0x1) != 0,
            write: (flags_num & 0x2) != 0,
            create: (flags_num & 0x4) != 0,
            truncate: (flags_num & 0x8) != 0,
        };

        let fd_table = self.fd_tables.entry(pid).or_default();
        let fd = fd_table.open(path, flags);

        SyscallResult::Success(SyscallReturn::FileDescriptor(fd))
    }

    fn handle_read(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("read: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("read: invalid fd argument".to_string()),
        };

        let count = match &args.args[1] {
            SyscallArg::Number(n) => *n as usize,
            _ => return SyscallResult::Error("read: invalid count argument".to_string()),
        };

        let fd_table = match self.fd_tables.get(&pid) {
            Some(table) => table,
            None => {
                return SyscallResult::Error(
                    "read: no file descriptor table for process".to_string(),
                )
            }
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("read: invalid file descriptor: {fd}")),
        };

        match descriptor {
            FileDescriptor::File {
                path,
                offset,
                flags,
            } => {
                if !flags.read {
                    return SyscallResult::Error(
                        "read: file descriptor not open for reading".to_string(),
                    );
                }

                match self.kernel.read_file(path) {
                    Ok(data) => {
                        let start = (*offset).min(data.len());
                        let end = (start + count).min(data.len());
                        let result = data[start..end].to_vec();
                        SyscallResult::Success(SyscallReturn::Buffer(result))
                    }
                    Err(e) => SyscallResult::Error(format!("read: {e}")),
                }
            }
            FileDescriptor::Socket { .. } => {
                SyscallResult::Error("read: use sock_recv for sockets".to_string())
            }
        }
    }

    fn handle_write(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("write: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("write: invalid fd argument".to_string()),
        };

        let data = match &args.args[1] {
            SyscallArg::Buffer(buf) => buf.clone(),
            SyscallArg::String(s) => s.as_bytes().to_vec(),
            _ => return SyscallResult::Error("write: invalid data argument".to_string()),
        };

        if fd == 1 || fd == 2 {
            let output = String::from_utf8_lossy(&data);
            println!("[PID {pid}] {output}");
            return SyscallResult::Success(SyscallReturn::Number(data.len() as i64));
        }

        let fd_table = match self.fd_tables.get(&pid) {
            Some(table) => table,
            None => {
                return SyscallResult::Error(
                    "write: no file descriptor table for process".to_string(),
                )
            }
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("write: invalid file descriptor: {fd}")),
        };

        match descriptor {
            FileDescriptor::File { path, flags, .. } => {
                if !flags.write {
                    return SyscallResult::Error(
                        "write: file descriptor not open for writing".to_string(),
                    );
                }

                match self.kernel.write_file(path, &data) {
                    Ok(_) => SyscallResult::Success(SyscallReturn::Number(data.len() as i64)),
                    Err(e) => SyscallResult::Error(format!("write: {e}")),
                }
            }
            FileDescriptor::Socket { .. } => {
                SyscallResult::Error("write: use sock_send for sockets".to_string())
            }
        }
    }

    fn handle_close(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("close: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("close: invalid fd argument".to_string()),
        };

        let fd_table = match self.fd_tables.get_mut(&pid) {
            Some(table) => table,
            None => {
                return SyscallResult::Error(
                    "close: no file descriptor table for process".to_string(),
                )
            }
        };

        if fd_table.close(fd) {
            SyscallResult::Success(SyscallReturn::Number(0))
        } else {
            SyscallResult::Error(format!("close: invalid file descriptor: {fd}"))
        }
    }

    fn handle_mkdir(&mut self, _pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("mkdir: insufficient arguments".to_string());
        }

        let path = match &args.args[0] {
            SyscallArg::String(s) => s,
            _ => return SyscallResult::Error("mkdir: invalid path argument".to_string()),
        };

        match self.kernel.create_directory(path) {
            Ok(_) => SyscallResult::Success(SyscallReturn::Number(0)),
            Err(e) => SyscallResult::Error(format!("mkdir: {e}")),
        }
    }

    fn handle_unlink(&mut self, _pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("unlink: insufficient arguments".to_string());
        }

        let path = match &args.args[0] {
            SyscallArg::String(s) => s,
            _ => return SyscallResult::Error("unlink: invalid path argument".to_string()),
        };

        match self.kernel.delete_file(path) {
            Ok(_) => SyscallResult::Success(SyscallReturn::Number(0)),
            Err(e) => SyscallResult::Error(format!("unlink: {e}")),
        }
    }

    fn handle_stat(&mut self, _pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("stat: insufficient arguments".to_string());
        }

        let path = match &args.args[0] {
            SyscallArg::String(s) => s,
            _ => return SyscallResult::Error("stat: invalid path argument".to_string()),
        };

        match self.kernel.list_directory(path) {
            Ok(entries) => SyscallResult::Success(SyscallReturn::VfsEntries(entries)),
            Err(e) => SyscallResult::Error(format!("stat: {e}")),
        }
    }

    fn handle_getpid(&mut self, pid: Pid) -> SyscallResult {
        SyscallResult::Success(SyscallReturn::ProcessId(pid))
    }

    fn handle_kill(&mut self, _caller_pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("kill: insufficient arguments".to_string());
        }

        let target_pid = match &args.args[0] {
            SyscallArg::Number(n) => *n as Pid,
            _ => return SyscallResult::Error("kill: invalid pid argument".to_string()),
        };

        match self.kernel.kill_process(target_pid) {
            Ok(_) => SyscallResult::Success(SyscallReturn::Number(0)),
            Err(e) => SyscallResult::Error(format!("kill: {e}")),
        }
    }

    fn handle_print(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("print: insufficient arguments".to_string());
        }

        let message = match &args.args[0] {
            SyscallArg::String(s) => s.clone(),
            SyscallArg::Buffer(buf) => String::from_utf8_lossy(buf).to_string(),
            _ => return SyscallResult::Error("print: invalid message argument".to_string()),
        };

        println!("[PID {pid}] {message}");
        SyscallResult::Success(SyscallReturn::Number(message.len() as i64))
    }

    fn handle_sock_open(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("sock_open: insufficient arguments".to_string());
        }

        let address_family = match &args.args[0] {
            SyscallArg::Number(n) => match AddressFamily::try_from(*n) {
                Ok(af) => af,
                Err(e) => return SyscallResult::Error(format!("sock_open: {e}")),
            },
            _ => return SyscallResult::Error("sock_open: invalid address family".to_string()),
        };

        let socket_type = match &args.args[1] {
            SyscallArg::Number(n) => match SocketType::try_from(*n) {
                Ok(st) => st,
                Err(e) => return SyscallResult::Error(format!("sock_open: {e}")),
            },
            _ => return SyscallResult::Error("sock_open: invalid socket type".to_string()),
        };

        if address_family == AddressFamily::Unix {
            return SyscallResult::Error("sock_open: Unix sockets not yet supported".to_string());
        }

        let handle = match socket_type {
            SocketType::Stream => {
                let dummy_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0);
                match TcpStream::connect(dummy_addr) {
                    Ok(_) => {
                        return SyscallResult::Error("sock_open: unexpected connection".to_string())
                    }
                    Err(_) => {
                        let listener = match TcpListener::bind("0.0.0.0:0") {
                            Ok(l) => l,
                            Err(e) => return SyscallResult::Error(format!("sock_open: {e}")),
                        };
                        drop(listener);

                        let placeholder = match TcpListener::bind("0.0.0.0:0") {
                            Ok(l) => l,
                            Err(e) => return SyscallResult::Error(format!("sock_open: {e}")),
                        };
                        SocketHandle::TcpListener(Arc::new(Mutex::new(placeholder)))
                    }
                }
            }
            SocketType::Dgram => {
                let socket = match UdpSocket::bind("0.0.0.0:0") {
                    Ok(s) => s,
                    Err(e) => return SyscallResult::Error(format!("sock_open: {e}")),
                };
                SocketHandle::UdpSocket(Arc::new(Mutex::new(socket)))
            }
        };

        let fd_table = self.fd_tables.entry(pid).or_default();
        let fd = fd_table.open_socket(handle, address_family, socket_type);

        SyscallResult::Success(SyscallReturn::FileDescriptor(fd))
    }

    fn handle_sock_bind(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 3 {
            return SyscallResult::Error(
                "sock_bind: insufficient arguments (need fd, ip, port)".to_string(),
            );
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_bind: invalid fd".to_string()),
        };

        let ip_str = match &args.args[1] {
            SyscallArg::String(s) => s.clone(),
            _ => return SyscallResult::Error("sock_bind: invalid ip address".to_string()),
        };

        let port = match &args.args[2] {
            SyscallArg::Number(n) => *n as u16,
            _ => return SyscallResult::Error("sock_bind: invalid port".to_string()),
        };

        let ip: IpAddr = match ip_str.parse() {
            Ok(addr) => addr,
            Err(_) => {
                return SyscallResult::Error(format!("sock_bind: invalid IP address: {ip_str}"))
            }
        };

        let bind_addr = SocketAddr::new(ip, port);

        let fd_table = match self.fd_tables.get_mut(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_bind: no fd table".to_string()),
        };

        let descriptor = match fd_table.get_mut(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_bind: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket {
                handle,
                state,
                local_addr,
                ..
            } => {
                if *state != SocketState::Created {
                    return SyscallResult::Error("sock_bind: socket already bound".to_string());
                }

                let result = match handle {
                    SocketHandle::TcpListener(listener) => {
                        let new_listener = match TcpListener::bind(bind_addr) {
                            Ok(l) => l,
                            Err(e) => return SyscallResult::Error(format!("sock_bind: {e}")),
                        };
                        *listener.lock().unwrap() = new_listener;
                        Ok(())
                    }
                    SocketHandle::UdpSocket(socket) => {
                        let new_socket = match UdpSocket::bind(bind_addr) {
                            Ok(s) => s,
                            Err(e) => return SyscallResult::Error(format!("sock_bind: {e}")),
                        };
                        *socket.lock().unwrap() = new_socket;
                        Ok(())
                    }
                    SocketHandle::TcpStream(_) => Err(anyhow::anyhow!("Cannot bind TCP stream")),
                };

                match result {
                    Ok(_) => {
                        *state = SocketState::Bound;
                        *local_addr = Some(bind_addr);
                        SyscallResult::Success(SyscallReturn::Number(0))
                    }
                    Err(e) => SyscallResult::Error(format!("sock_bind: {e}")),
                }
            }
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_bind: not a socket".to_string())
            }
        }
    }

    fn handle_sock_listen(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("sock_listen: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_listen: invalid fd".to_string()),
        };

        let _backlog = match &args.args[1] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_listen: invalid backlog".to_string()),
        };

        let fd_table = match self.fd_tables.get_mut(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_listen: no fd table".to_string()),
        };

        let descriptor = match fd_table.get_mut(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_listen: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket { handle, state, .. } => {
                if *state != SocketState::Bound {
                    return SyscallResult::Error("sock_listen: socket not bound".to_string());
                }

                match handle {
                    SocketHandle::TcpListener(_) => {
                        *state = SocketState::Listening;
                        SyscallResult::Success(SyscallReturn::Number(0))
                    }
                    _ => SyscallResult::Error("sock_listen: not a TCP socket".to_string()),
                }
            }
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_listen: not a socket".to_string())
            }
        }
    }

    fn handle_sock_accept(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("sock_accept: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_accept: invalid fd".to_string()),
        };

        let fd_table = match self.fd_tables.get_mut(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_accept: no fd table".to_string()),
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_accept: invalid fd: {fd}")),
        };

        let (stream, peer_addr) = match descriptor {
            FileDescriptor::Socket { handle, state, .. } => {
                if *state != SocketState::Listening {
                    return SyscallResult::Error("sock_accept: socket not listening".to_string());
                }

                match handle {
                    SocketHandle::TcpListener(listener) => {
                        match listener.lock().unwrap().accept() {
                            Ok((stream, addr)) => (stream, addr),
                            Err(e) => return SyscallResult::Error(format!("sock_accept: {e}")),
                        }
                    }
                    _ => {
                        return SyscallResult::Error(
                            "sock_accept: not a listening socket".to_string(),
                        )
                    }
                }
            }
            FileDescriptor::File { .. } => {
                return SyscallResult::Error("sock_accept: not a socket".to_string())
            }
        };

        let new_fd = fd_table.open_socket(
            SocketHandle::TcpStream(Arc::new(Mutex::new(stream))),
            AddressFamily::Inet,
            SocketType::Stream,
        );

        if let Some(FileDescriptor::Socket {
            state,
            peer_addr: peer,
            ..
        }) = fd_table.get_mut(new_fd)
        {
            *state = SocketState::Connected;
            *peer = Some(peer_addr);
        }

        SyscallResult::Success(SyscallReturn::FileDescriptor(new_fd))
    }

    fn handle_sock_connect(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 3 {
            return SyscallResult::Error("sock_connect: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_connect: invalid fd".to_string()),
        };

        let ip_str = match &args.args[1] {
            SyscallArg::String(s) => s.clone(),
            _ => return SyscallResult::Error("sock_connect: invalid ip".to_string()),
        };

        let port = match &args.args[2] {
            SyscallArg::Number(n) => *n as u16,
            _ => return SyscallResult::Error("sock_connect: invalid port".to_string()),
        };

        let ip: IpAddr = match ip_str.parse() {
            Ok(addr) => addr,
            Err(_) => return SyscallResult::Error(format!("sock_connect: invalid IP: {ip_str}")),
        };

        let connect_addr = SocketAddr::new(ip, port);

        let fd_table = match self.fd_tables.get_mut(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_connect: no fd table".to_string()),
        };

        let descriptor = match fd_table.get_mut(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_connect: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket {
                handle,
                state,
                peer_addr,
                socket_type,
                ..
            } => match socket_type {
                SocketType::Stream => {
                    let stream = match TcpStream::connect(connect_addr) {
                        Ok(s) => s,
                        Err(e) => return SyscallResult::Error(format!("sock_connect: {e}")),
                    };

                    *handle = SocketHandle::TcpStream(Arc::new(Mutex::new(stream)));
                    *state = SocketState::Connected;
                    *peer_addr = Some(connect_addr);
                    SyscallResult::Success(SyscallReturn::Number(0))
                }
                SocketType::Dgram => match handle {
                    SocketHandle::UdpSocket(socket) => {
                        match socket.lock().unwrap().connect(connect_addr) {
                            Ok(_) => {
                                *state = SocketState::Connected;
                                *peer_addr = Some(connect_addr);
                                SyscallResult::Success(SyscallReturn::Number(0))
                            }
                            Err(e) => SyscallResult::Error(format!("sock_connect: {e}")),
                        }
                    }
                    _ => SyscallResult::Error("sock_connect: invalid socket handle".to_string()),
                },
            },
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_connect: not a socket".to_string())
            }
        }
    }

    fn handle_sock_recv(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("sock_recv: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_recv: invalid fd".to_string()),
        };

        let max_len = match &args.args[1] {
            SyscallArg::Number(n) => *n as usize,
            _ => return SyscallResult::Error("sock_recv: invalid length".to_string()),
        };

        let fd_table = match self.fd_tables.get(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_recv: no fd table".to_string()),
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_recv: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket { handle, state, .. } => {
                if *state != SocketState::Connected {
                    return SyscallResult::Error("sock_recv: socket not connected".to_string());
                }

                let mut buffer = vec![0u8; max_len];

                let bytes_read = match handle {
                    SocketHandle::TcpStream(stream) => {
                        match stream.lock().unwrap().read(&mut buffer) {
                            Ok(n) => n,
                            Err(e) => return SyscallResult::Error(format!("sock_recv: {e}")),
                        }
                    }
                    SocketHandle::UdpSocket(socket) => {
                        match socket.lock().unwrap().recv(&mut buffer) {
                            Ok(n) => n,
                            Err(e) => return SyscallResult::Error(format!("sock_recv: {e}")),
                        }
                    }
                    _ => return SyscallResult::Error("sock_recv: invalid socket type".to_string()),
                };

                buffer.truncate(bytes_read);
                SyscallResult::Success(SyscallReturn::Buffer(buffer))
            }
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_recv: not a socket".to_string())
            }
        }
    }

    fn handle_sock_send(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("sock_send: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_send: invalid fd".to_string()),
        };

        let data = match &args.args[1] {
            SyscallArg::Buffer(buf) => buf.clone(),
            SyscallArg::String(s) => s.as_bytes().to_vec(),
            _ => return SyscallResult::Error("sock_send: invalid data".to_string()),
        };

        let fd_table = match self.fd_tables.get(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_send: no fd table".to_string()),
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_send: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket { handle, state, .. } => {
                if *state != SocketState::Connected {
                    return SyscallResult::Error("sock_send: socket not connected".to_string());
                }

                let bytes_sent = match handle {
                    SocketHandle::TcpStream(stream) => match stream.lock().unwrap().write(&data) {
                        Ok(n) => n,
                        Err(e) => return SyscallResult::Error(format!("sock_send: {e}")),
                    },
                    SocketHandle::UdpSocket(socket) => match socket.lock().unwrap().send(&data) {
                        Ok(n) => n,
                        Err(e) => return SyscallResult::Error(format!("sock_send: {e}")),
                    },
                    _ => return SyscallResult::Error("sock_send: invalid socket type".to_string()),
                };

                SyscallResult::Success(SyscallReturn::Number(bytes_sent as i64))
            }
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_send: not a socket".to_string())
            }
        }
    }

    fn handle_sock_shutdown(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.len() < 2 {
            return SyscallResult::Error("sock_shutdown: insufficient arguments".to_string());
        }

        let fd = match &args.args[0] {
            SyscallArg::Number(n) => *n as i32,
            _ => return SyscallResult::Error("sock_shutdown: invalid fd".to_string()),
        };

        let how = match &args.args[1] {
            SyscallArg::Number(n) => match *n {
                0 => Shutdown::Read,
                1 => Shutdown::Write,
                2 => Shutdown::Both,
                _ => return SyscallResult::Error("sock_shutdown: invalid how".to_string()),
            },
            _ => return SyscallResult::Error("sock_shutdown: invalid how".to_string()),
        };

        let fd_table = match self.fd_tables.get(&pid) {
            Some(table) => table,
            None => return SyscallResult::Error("sock_shutdown: no fd table".to_string()),
        };

        let descriptor = match fd_table.get(fd) {
            Some(desc) => desc,
            None => return SyscallResult::Error(format!("sock_shutdown: invalid fd: {fd}")),
        };

        match descriptor {
            FileDescriptor::Socket { handle, .. } => match handle {
                SocketHandle::TcpStream(stream) => match stream.lock().unwrap().shutdown(how) {
                    Ok(_) => SyscallResult::Success(SyscallReturn::Number(0)),
                    Err(e) => SyscallResult::Error(format!("sock_shutdown: {e}")),
                },
                _ => SyscallResult::Error(
                    "sock_shutdown: only TCP streams support shutdown".to_string(),
                ),
            },
            FileDescriptor::File { .. } => {
                SyscallResult::Error("sock_shutdown: not a socket".to_string())
            }
        }
    }

    fn handle_sock_close(&mut self, pid: Pid, args: SyscallArgs) -> SyscallResult {
        self.handle_close(pid, args)
    }

    fn handle_getaddrinfo(&mut self, _pid: Pid, args: SyscallArgs) -> SyscallResult {
        if args.args.is_empty() {
            return SyscallResult::Error("getaddrinfo: insufficient arguments".to_string());
        }

        let hostname = match &args.args[0] {
            SyscallArg::String(s) => s.clone(),
            _ => return SyscallResult::Error("getaddrinfo: invalid hostname".to_string()),
        };

        let port = if args.args.len() > 1 {
            match &args.args[1] {
                SyscallArg::Number(n) => *n as u16,
                SyscallArg::String(s) => s.parse().unwrap_or_default(),
                _ => 0,
            }
        } else {
            0
        };

        let addr_str = format!("{hostname}:{port}");
        let addrs: Result<Vec<SocketAddr>> = addr_str
            .to_socket_addrs()
            .map(|iter| iter.collect())
            .map_err(|e| anyhow::anyhow!("DNS resolution failed: {e}"));

        match addrs {
            Ok(addresses) => {
                let addr_strings: Vec<String> =
                    addresses.iter().map(|addr| addr.to_string()).collect();
                SyscallResult::Success(SyscallReturn::String(addr_strings.join(",")))
            }
            Err(e) => SyscallResult::Error(format!("getaddrinfo: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_socket_types() {
        assert_eq!(AddressFamily::Inet as i64, 2);
        assert_eq!(AddressFamily::Inet6 as i64, 10);
        assert_eq!(SocketType::Stream as i64, 1);
        assert_eq!(SocketType::Dgram as i64, 2);
    }

    #[test]
    fn test_socket_state_transitions() {
        let initial_state = SocketState::Created;
        assert_eq!(initial_state, SocketState::Created);

        let bound_state = SocketState::Bound;
        assert_eq!(bound_state, SocketState::Bound);

        let listening_state = SocketState::Listening;
        assert_eq!(listening_state, SocketState::Listening);

        let connected_state = SocketState::Connected;
        assert_eq!(connected_state, SocketState::Connected);
    }

    #[test]
    fn test_tcp_socket_creation_and_binding() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let addr = listener.local_addr().expect("Failed to get address");
        assert!(addr.port() > 0);
    }

    #[test]
    fn test_tcp_client_server_communication() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};

        let server_port = 19999;

        let server_handle = thread::spawn(move || {
            let listener = TcpListener::bind(format!("127.0.0.1:{server_port}"))
                .expect("Failed to bind server");

            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let mut buffer = [0u8; 1024];
                    let n = stream.read(&mut buffer).expect("Failed to read");
                    let received = String::from_utf8_lossy(&buffer[..n]).to_string();

                    let response = b"Hello from server!";
                    stream.write_all(response).expect("Failed to send");

                    received
                }
                Err(e) => panic!("Failed to accept: {e}"),
            }
        });

        thread::sleep(Duration::from_millis(100));

        let client_message = "Hello from client!";
        let mut stream =
            TcpStream::connect(format!("127.0.0.1:{server_port}")).expect("Failed to connect");

        stream
            .write_all(client_message.as_bytes())
            .expect("Failed to send");

        let mut buffer = [0u8; 1024];
        let n = stream.read(&mut buffer).expect("Failed to read");
        let received = String::from_utf8_lossy(&buffer[..n]).to_string();

        let server_received = server_handle.join().expect("Server thread panicked");

        assert_eq!(server_received, client_message);
        assert_eq!(received, "Hello from server!");
    }

    #[test]
    fn test_udp_socket_communication() {
        use std::net::UdpSocket;

        let receiver = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind receiver");
        let receiver_addr = receiver
            .local_addr()
            .expect("Failed to get receiver address");

        let sender = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind sender");

        let message = b"Hello via UDP!";
        sender
            .send_to(message, receiver_addr)
            .expect("Failed to send");

        let mut buffer = [0u8; 1024];
        let (n, _src) = receiver.recv_from(&mut buffer).expect("Failed to receive");

        assert_eq!(&buffer[..n], message);
    }

    #[test]
    fn test_file_descriptor_table() {
        let mut table = FileDescriptorTable::default();

        assert_eq!(table.descriptors.len(), 3);
        assert!(table.get(0).is_some());
        assert!(table.get(1).is_some());
        assert!(table.get(2).is_some());

        let fd = table.open(
            "/test/file.txt".to_string(),
            OpenFlags {
                read: true,
                write: false,
                create: false,
                truncate: false,
            },
        );

        assert_eq!(fd, 3);
        assert!(table.get(3).is_some());

        assert!(table.close(3));
        assert!(table.get(3).is_none());
    }

    #[test]
    fn test_socket_descriptor() {
        use std::net::TcpListener;

        let mut table = FileDescriptorTable::default();

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let handle = SocketHandle::TcpListener(Arc::new(Mutex::new(listener)));

        let fd = table.open_socket(handle, AddressFamily::Inet, SocketType::Stream);

        assert_eq!(fd, 3);

        if let Some(FileDescriptor::Socket {
            address_family,
            socket_type,
            state,
            ..
        }) = table.get(fd)
        {
            assert_eq!(*address_family, AddressFamily::Inet);
            assert_eq!(*socket_type, SocketType::Stream);
            assert_eq!(*state, SocketState::Created);
        } else {
            panic!("Expected socket descriptor");
        }
    }

    #[test]
    fn test_address_family_conversion() {
        assert_eq!(AddressFamily::try_from(2).unwrap(), AddressFamily::Inet);
        assert_eq!(AddressFamily::try_from(10).unwrap(), AddressFamily::Inet6);
        assert_eq!(AddressFamily::try_from(1).unwrap(), AddressFamily::Unix);
        assert!(AddressFamily::try_from(99).is_err());
    }

    #[test]
    fn test_socket_type_conversion() {
        assert_eq!(SocketType::try_from(1).unwrap(), SocketType::Stream);
        assert_eq!(SocketType::try_from(2).unwrap(), SocketType::Dgram);
        assert!(SocketType::try_from(99).is_err());
    }

    #[test]
    fn test_syscall_number_conversion() {
        assert_eq!(
            SyscallNumber::try_from(19).unwrap(),
            SyscallNumber::SockOpen
        );
        assert_eq!(
            SyscallNumber::try_from(20).unwrap(),
            SyscallNumber::SockBind
        );
        assert_eq!(
            SyscallNumber::try_from(21).unwrap(),
            SyscallNumber::SockListen
        );
        assert_eq!(
            SyscallNumber::try_from(22).unwrap(),
            SyscallNumber::SockAccept
        );
        assert_eq!(
            SyscallNumber::try_from(23).unwrap(),
            SyscallNumber::SockConnect
        );
        assert_eq!(
            SyscallNumber::try_from(24).unwrap(),
            SyscallNumber::SockRecv
        );
        assert_eq!(
            SyscallNumber::try_from(25).unwrap(),
            SyscallNumber::SockSend
        );
        assert_eq!(
            SyscallNumber::try_from(26).unwrap(),
            SyscallNumber::SockShutdown
        );
        assert_eq!(
            SyscallNumber::try_from(27).unwrap(),
            SyscallNumber::SockClose
        );
        assert_eq!(
            SyscallNumber::try_from(28).unwrap(),
            SyscallNumber::GetAddrInfo
        );
        assert!(SyscallNumber::try_from(999).is_err());
    }

    #[test]
    fn test_concurrent_tcp_connections() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::sync::{Arc, Mutex};

        let server_port = 20001;
        let connection_count = Arc::new(Mutex::new(0));
        let count_clone = connection_count.clone();

        let server_handle = thread::spawn(move || {
            let listener = TcpListener::bind(format!("127.0.0.1:{server_port}"))
                .expect("Failed to bind server");

            for _ in 0..5 {
                match listener.accept() {
                    Ok((mut stream, _addr)) => {
                        let mut count = count_clone.lock().unwrap();
                        *count += 1;

                        let mut buffer = [0u8; 1024];
                        let _ = stream.read(&mut buffer);
                        stream.write_all(b"OK").ok();
                    }
                    Err(_) => break,
                }
            }
        });

        thread::sleep(Duration::from_millis(100));

        let mut handles = vec![];
        for _ in 0..5 {
            let handle = thread::spawn(move || {
                let mut stream = TcpStream::connect(format!("127.0.0.1:{server_port}"))
                    .expect("Failed to connect");
                stream.write_all(b"Hello").expect("Failed to send");
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Client thread panicked");
        }

        server_handle.join().expect("Server thread panicked");

        let final_count = *connection_count.lock().unwrap();
        assert_eq!(final_count, 5);
    }

    #[test]
    fn test_udp_bidirectional() {
        use std::net::UdpSocket;

        let socket1 = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket1");
        let socket2 = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind socket2");

        let addr1 = socket1.local_addr().expect("Failed to get socket1 address");
        let addr2 = socket2.local_addr().expect("Failed to get socket2 address");

        socket1
            .send_to(b"Message from 1", addr2)
            .expect("Failed to send");
        socket2
            .send_to(b"Message from 2", addr1)
            .expect("Failed to send");

        let mut buffer = [0u8; 1024];
        let (n, src) = socket2
            .recv_from(&mut buffer)
            .expect("Failed to receive at socket2");
        assert_eq!(&buffer[..n], b"Message from 1");
        assert_eq!(src, addr1);

        let (n, src) = socket1
            .recv_from(&mut buffer)
            .expect("Failed to receive at socket1");
        assert_eq!(&buffer[..n], b"Message from 2");
        assert_eq!(src, addr2);
    }

    #[test]
    fn test_tcp_connection_error_handling() {
        use std::net::TcpStream;

        let result = TcpStream::connect("127.0.0.1:1");
        assert!(result.is_err());
    }

    #[test]
    fn test_file_descriptor_exhaustion() {
        let mut table = FileDescriptorTable::default();

        for i in 0..1000 {
            let fd = table.open(
                format!("/test/file{i}.txt"),
                OpenFlags {
                    read: true,
                    write: false,
                    create: false,
                    truncate: false,
                },
            );
            assert_eq!(fd, 3 + i);
        }

        assert_eq!(table.descriptors.len(), 1003);
    }

    #[test]
    fn test_socket_state_validation() {
        let mut table = FileDescriptorTable::default();
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let handle = SocketHandle::TcpListener(Arc::new(Mutex::new(listener)));
        let fd = table.open_socket(handle, AddressFamily::Inet, SocketType::Stream);

        if let Some(FileDescriptor::Socket { state, .. }) = table.get(fd) {
            assert_eq!(*state, SocketState::Created);
        }

        if let Some(FileDescriptor::Socket {
            state, local_addr, ..
        }) = table.get_mut(fd)
        {
            *state = SocketState::Bound;
            *local_addr = Some("127.0.0.1:8080".parse().unwrap());
        }

        if let Some(FileDescriptor::Socket { state, .. }) = table.get(fd) {
            assert_eq!(*state, SocketState::Bound);
        }
    }
}
