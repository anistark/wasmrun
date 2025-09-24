use crate::runtime::microkernel::{Pid, SyscallInterface, VfsEntry, WasmMicroKernel};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
            _ => Err(anyhow::anyhow!("Unknown syscall number: {}", value)),
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

/// File descriptor table for a process
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileDescriptorTable {
    descriptors: HashMap<i32, FileDescriptor>,
    next_fd: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    pub path: String,
    pub offset: usize,
    pub flags: OpenFlags,
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
            next_fd: 3, // 0, 1, 2 reserved for stdin, stdout, stderr
        };

        // Add standard descriptors
        table.descriptors.insert(
            0,
            FileDescriptor {
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
            FileDescriptor {
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
            FileDescriptor {
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
            FileDescriptor {
                path,
                offset: 0,
                flags,
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
            _ => SyscallResult::Error(format!("Unimplemented syscall: {:?}", syscall)),
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

        if !descriptor.flags.read {
            return SyscallResult::Error("read: file descriptor not open for reading".to_string());
        }

        match self.kernel.read_file(&descriptor.path) {
            Ok(data) => {
                let start = descriptor.offset.min(data.len());
                let end = (start + count).min(data.len());
                let result = data[start..end].to_vec();
                SyscallResult::Success(SyscallReturn::Buffer(result))
            }
            Err(e) => SyscallResult::Error(format!("read: {e}")),
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

        if !descriptor.flags.write {
            return SyscallResult::Error("write: file descriptor not open for writing".to_string());
        }

        // Handle stdout/stderr specially
        if fd == 1 || fd == 2 {
            // In a real implementation, this would go to the browser console
            let output = String::from_utf8_lossy(&data);
            println!("[PID {pid}] {output}");
            return SyscallResult::Success(SyscallReturn::Number(data.len() as i64));
        }

        match self.kernel.write_file(&descriptor.path, &data) {
            Ok(_) => SyscallResult::Success(SyscallReturn::Number(data.len() as i64)),
            Err(e) => SyscallResult::Error(format!("write: {e}")),
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
}
