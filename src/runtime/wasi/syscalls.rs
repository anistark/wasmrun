//! WASI syscall implementations that operate on linear memory.

use crate::runtime::core::memory::LinearMemory;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{FdKind, WasiEnv, WASI_STDERR_FD, WASI_STDIN_FD, WASI_STDOUT_FD};

pub const WASI_ESUCCESS: i32 = 0;
pub const WASI_EBADF: i32 = 8;
pub const WASI_EEXIST: i32 = 20;
pub const WASI_EINVAL: i32 = 28;
pub const WASI_EIO: i32 = 29;
pub const WASI_EISDIR: i32 = 31;
pub const WASI_ENOENT: i32 = 44;
pub const WASI_ENOSYS: i32 = 52;
pub const WASI_ENOTDIR: i32 = 54;
pub const WASI_ENOTEMPTY: i32 = 55;

pub const WASI_CLOCK_REALTIME: u32 = 0;
pub const WASI_CLOCK_MONOTONIC: u32 = 1;

pub const WASI_FILETYPE_UNKNOWN: u8 = 0;
pub const WASI_FILETYPE_CHARACTER_DEVICE: u8 = 2;
pub const WASI_FILETYPE_DIRECTORY: u8 = 3;
pub const WASI_FILETYPE_REGULAR_FILE: u8 = 4;
pub const WASI_FILETYPE_SYMBOLIC_LINK: u8 = 7;

const WASI_O_CREAT: u32 = 1;
const WASI_O_DIRECTORY: u32 = 2;
const WASI_O_EXCL: u32 = 4;
const WASI_O_TRUNC: u32 = 8;

const WASI_WHENCE_SET: u32 = 0;
const WASI_WHENCE_CUR: u32 = 1;
const WASI_WHENCE_END: u32 = 2;

fn read_guest_string(ptr: u32, len: u32, memory: &LinearMemory) -> Result<String, i32> {
    let bytes = memory
        .read_bytes(ptr as usize, len as usize)
        .map_err(|_| WASI_EINVAL)?;
    std::str::from_utf8(&bytes)
        .map(|s| s.to_string())
        .map_err(|_| WASI_EINVAL)
}

fn write_file_at(
    path: &std::path::Path,
    offset: u64,
    data: &[u8],
) -> Result<usize, std::io::Error> {
    use std::io::Write;
    let mut content = if path.exists() {
        std::fs::read(path)?
    } else {
        Vec::new()
    };
    let off = offset as usize;
    if off + data.len() > content.len() {
        content.resize(off + data.len(), 0);
    }
    content[off..off + data.len()].copy_from_slice(data);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&content)?;
    Ok(data.len())
}

// ── I/O syscalls ──────────────────────────────────────────────────────

pub fn fd_write(
    fd: u32,
    iovs_ptr: u32,
    iovs_len: u32,
    nwritten_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let mut total_written: u32 = 0;

    for i in 0..iovs_len {
        let iov_base = iovs_ptr as usize + (i as usize) * 8;
        let buf_ptr = match memory.read_i32(iov_base) {
            Ok(v) => v as u32 as usize,
            Err(_) => return WASI_EINVAL,
        };
        let buf_len = match memory.read_i32(iov_base + 4) {
            Ok(v) => v as u32 as usize,
            Err(_) => return WASI_EINVAL,
        };
        if buf_len == 0 {
            continue;
        }
        let bytes = match memory.read_bytes(buf_ptr, buf_len) {
            Ok(b) => b,
            Err(_) => return WASI_EINVAL,
        };

        match fd {
            WASI_STDOUT_FD => {
                if let Ok(mut e) = env.lock() {
                    e.stdout_mut().extend_from_slice(&bytes);
                }
            }
            WASI_STDERR_FD => {
                if let Ok(mut e) = env.lock() {
                    e.stderr_mut().extend_from_slice(&bytes);
                }
            }
            _ => {
                let mut e = match env.lock() {
                    Ok(e) => e,
                    Err(_) => return WASI_EIO,
                };
                let (host_path, offset) = match e.get_fd(fd) {
                    Some(entry) if entry.kind == FdKind::File => {
                        (entry.host_path.clone(), entry.offset)
                    }
                    Some(_) => return WASI_EISDIR,
                    None => return WASI_EBADF,
                };
                match write_file_at(&host_path, offset, &bytes) {
                    Ok(n) => {
                        if let Some(fe) = e.get_fd_mut(fd) {
                            fe.offset += n as u64;
                        }
                    }
                    Err(_) => return WASI_EIO,
                }
            }
        }
        total_written += bytes.len() as u32;
    }

    if memory
        .write_i32(nwritten_ptr as usize, total_written as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn fd_read(
    fd: u32,
    iovs_ptr: u32,
    iovs_len: u32,
    nread_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    if fd == WASI_STDIN_FD {
        return if memory.write_i32(nread_ptr as usize, 0).is_ok() {
            WASI_ESUCCESS
        } else {
            WASI_EINVAL
        };
    }
    if fd == WASI_STDOUT_FD || fd == WASI_STDERR_FD {
        return WASI_EBADF;
    }

    let mut e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let (host_path, offset) = match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::File => (entry.host_path.clone(), entry.offset),
        Some(_) => return WASI_EISDIR,
        None => return WASI_EBADF,
    };

    let file_data = match std::fs::read(&host_path) {
        Ok(d) => d,
        Err(_) => return WASI_EIO,
    };

    let mut total_read: u32 = 0;
    let mut cur_offset = offset as usize;

    for i in 0..iovs_len {
        let iov_base = iovs_ptr as usize + (i as usize) * 8;
        let buf_ptr = match memory.read_i32(iov_base) {
            Ok(v) => v as u32 as usize,
            Err(_) => return WASI_EINVAL,
        };
        let buf_len = match memory.read_i32(iov_base + 4) {
            Ok(v) => v as u32 as usize,
            Err(_) => return WASI_EINVAL,
        };
        if cur_offset >= file_data.len() {
            break;
        }
        let available = (file_data.len() - cur_offset).min(buf_len);
        if memory
            .write_bytes(buf_ptr, &file_data[cur_offset..cur_offset + available])
            .is_err()
        {
            return WASI_EINVAL;
        }
        cur_offset += available;
        total_read += available as u32;
    }

    if let Some(fe) = e.get_fd_mut(fd) {
        fe.offset = cur_offset as u64;
    }

    if memory
        .write_i32(nread_ptr as usize, total_read as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn fd_close(fd: u32, env: &Arc<Mutex<WasiEnv>>) -> i32 {
    match env.lock() {
        Ok(mut e) => {
            if e.close_fd(fd) {
                WASI_ESUCCESS
            } else {
                WASI_EBADF
            }
        }
        Err(_) => WASI_EIO,
    }
}

pub fn fd_seek(
    fd: u32,
    offset: i64,
    whence: u32,
    newoffset_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    if fd <= WASI_STDERR_FD {
        return if memory.write_i64(newoffset_ptr as usize, 0).is_ok() {
            WASI_ESUCCESS
        } else {
            WASI_EINVAL
        };
    }

    let mut e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::File => entry.clone(),
        Some(_) => return WASI_EINVAL,
        None => return WASI_EBADF,
    };

    let file_size = std::fs::metadata(&entry.host_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let new_offset = match whence {
        WASI_WHENCE_SET => offset.max(0),
        WASI_WHENCE_CUR => (entry.offset as i64 + offset).max(0),
        WASI_WHENCE_END => (file_size + offset).max(0),
        _ => return WASI_EINVAL,
    };

    if let Some(fe) = e.get_fd_mut(fd) {
        fe.offset = new_offset as u64;
    }

    if memory
        .write_i64(newoffset_ptr as usize, new_offset)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

// ── fd stat syscalls ──────────────────────────────────────────────────

pub fn fd_fdstat_get(
    fd: u32,
    stat_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) => entry,
        None => return WASI_EBADF,
    };

    let (filetype, flags, rights) = match entry.kind {
        FdKind::Stdin => (WASI_FILETYPE_CHARACTER_DEVICE, 0u16, 0x200u64),
        FdKind::Stdout | FdKind::Stderr => (WASI_FILETYPE_CHARACTER_DEVICE, 1u16, 0x400u64),
        FdKind::PreopenDir | FdKind::Directory => (WASI_FILETYPE_DIRECTORY, 0u16, 0x0FFF_FFFFu64),
        FdKind::File => (WASI_FILETYPE_REGULAR_FILE, 0u16, 0x0FFF_FFFFu64),
    };

    let base = stat_ptr as usize;
    // fdstat struct: 24 bytes
    for i in 0..24 {
        if memory.write_u8(base + i, 0).is_err() {
            return WASI_EINVAL;
        }
    }
    if memory.write_u8(base, filetype).is_err()
        || memory.write_u16(base + 2, flags).is_err()
        || memory.write_i64(base + 8, rights as i64).is_err()
        || memory.write_i64(base + 16, rights as i64).is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn fd_filestat_get(
    fd: u32,
    buf_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) => entry.clone(),
        None => return WASI_EBADF,
    };

    match entry.kind {
        FdKind::Stdin | FdKind::Stdout | FdKind::Stderr => {
            let base = buf_ptr as usize;
            for i in 0..64 {
                if memory.write_u8(base + i, 0).is_err() {
                    return WASI_EINVAL;
                }
            }
            if memory
                .write_u8(base + 16, WASI_FILETYPE_CHARACTER_DEVICE)
                .is_err()
            {
                return WASI_EINVAL;
            }
            WASI_ESUCCESS
        }
        _ => match std::fs::metadata(&entry.host_path) {
            Ok(m) => write_filestat(buf_ptr, &m, memory),
            Err(_) => WASI_EIO,
        },
    }
}

// ── preopen syscalls ──────────────────────────────────────────────────

pub fn fd_prestat_get(
    fd: u32,
    buf_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::PreopenDir => entry,
        _ => return WASI_EBADF,
    };

    let name_len = entry.guest_path.len() as i32;
    let base = buf_ptr as usize;

    // prestat struct (8 bytes): u8 tag at 0, u32 name_len at 4
    if memory.write_u8(base, 0).is_err() {
        return WASI_EINVAL;
    }
    // padding bytes 1-3
    for i in 1..4 {
        if memory.write_u8(base + i, 0).is_err() {
            return WASI_EINVAL;
        }
    }
    if memory.write_i32(base + 4, name_len).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn fd_prestat_dir_name(
    fd: u32,
    buf_ptr: u32,
    buf_len: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::PreopenDir => entry,
        _ => return WASI_EBADF,
    };

    let name = entry.guest_path.as_bytes();
    let write_len = (buf_len as usize).min(name.len());
    if memory
        .write_bytes(buf_ptr as usize, &name[..write_len])
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

// ── path syscalls ─────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn path_open(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    oflags: u32,
    _fdflags: u32,
    fd_out_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let mut e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let host_path = match e.resolve_path(dir_fd, &path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    let creat = oflags & WASI_O_CREAT != 0;
    let directory = oflags & WASI_O_DIRECTORY != 0;
    let excl = oflags & WASI_O_EXCL != 0;
    let trunc = oflags & WASI_O_TRUNC != 0;

    if excl && host_path.exists() {
        return WASI_EEXIST;
    }

    if directory {
        if host_path.exists() && !host_path.is_dir() {
            return WASI_ENOTDIR;
        }
        if !host_path.exists() {
            if creat {
                if std::fs::create_dir_all(&host_path).is_err() {
                    return WASI_EIO;
                }
            } else {
                return WASI_ENOENT;
            }
        }
    } else if creat && !host_path.exists() {
        if let Some(parent) = host_path.parent() {
            if !parent.exists() && std::fs::create_dir_all(parent).is_err() {
                return WASI_EIO;
            }
        }
        if std::fs::File::create(&host_path).is_err() {
            return WASI_EIO;
        }
    } else if !host_path.exists() {
        return WASI_ENOENT;
    }

    if trunc && host_path.is_file() && std::fs::File::create(&host_path).is_err() {
        return WASI_EIO;
    }

    let kind = if host_path.is_dir() {
        FdKind::Directory
    } else {
        FdKind::File
    };

    let fd = e.allocate_fd(super::FdEntry {
        kind,
        host_path,
        guest_path: path,
        offset: 0,
        flags: 0,
    });

    if memory.write_i32(fd_out_ptr as usize, fd as i32).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn path_filestat_get(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    buf_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let host_path = match e.resolve_path(dir_fd, &path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    match std::fs::metadata(&host_path) {
        Ok(m) => write_filestat(buf_ptr, &m, memory),
        Err(_) => WASI_ENOENT,
    }
}

pub fn path_create_directory(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let host_path = match e.resolve_path(dir_fd, &path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    if host_path.exists() {
        return WASI_EEXIST;
    }

    match std::fs::create_dir_all(&host_path) {
        Ok(_) => WASI_ESUCCESS,
        Err(_) => WASI_EIO,
    }
}

pub fn path_unlink_file(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let host_path = match e.resolve_path(dir_fd, &path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    if !host_path.exists() {
        return WASI_ENOENT;
    }
    if host_path.is_dir() {
        return WASI_EISDIR;
    }

    match std::fs::remove_file(&host_path) {
        Ok(_) => WASI_ESUCCESS,
        Err(_) => WASI_EIO,
    }
}

pub fn path_remove_directory(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let host_path = match e.resolve_path(dir_fd, &path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    if !host_path.exists() {
        return WASI_ENOENT;
    }
    if !host_path.is_dir() {
        return WASI_ENOTDIR;
    }

    match std::fs::remove_dir(&host_path) {
        Ok(_) => WASI_ESUCCESS,
        Err(e) => {
            if e.to_string().contains("not empty")
                || e.kind() == std::io::ErrorKind::DirectoryNotEmpty
            {
                WASI_ENOTEMPTY
            } else {
                WASI_EIO
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn path_rename(
    old_fd: u32,
    old_path_ptr: u32,
    old_path_len: u32,
    new_fd: u32,
    new_path_ptr: u32,
    new_path_len: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let old_path = match read_guest_string(old_path_ptr, old_path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let new_path = match read_guest_string(new_path_ptr, new_path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let old_host = match e.resolve_path(old_fd, &old_path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };
    let new_host = match e.resolve_path(new_fd, &new_path) {
        Ok(p) => p,
        Err(_) => return WASI_EBADF,
    };

    if !old_host.exists() {
        return WASI_ENOENT;
    }

    match std::fs::rename(&old_host, &new_host) {
        Ok(_) => WASI_ESUCCESS,
        Err(_) => WASI_EIO,
    }
}

// ── directory reading ─────────────────────────────────────────────────

pub fn fd_readdir(
    fd: u32,
    buf_ptr: u32,
    buf_len: u32,
    cookie: u64,
    bufused_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let entry = match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::PreopenDir || entry.kind == FdKind::Directory => {
            entry.clone()
        }
        Some(_) => return WASI_ENOTDIR,
        None => return WASI_EBADF,
    };

    let entries: Vec<_> = match std::fs::read_dir(&entry.host_path) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return WASI_EIO,
    };

    // dirent: 24-byte header + name bytes (no NUL)
    //   0: d_next (u64)
    //   8: d_ino (u64)
    //  16: d_namlen (u32)
    //  20: d_type (u8)
    let mut offset = 0u32;
    for (i, dir_entry) in entries.iter().enumerate().skip(cookie as usize) {
        let name = dir_entry.file_name();
        let name_bytes = name.to_string_lossy();
        let name_bytes = name_bytes.as_bytes();
        let entry_size = 24 + name_bytes.len() as u32;

        if offset + entry_size > buf_len {
            break;
        }

        let base = buf_ptr as usize + offset as usize;
        if memory.write_i64(base, (i + 1) as i64).is_err()
            || memory.write_i64(base + 8, 0).is_err()
            || memory
                .write_i32(base + 16, name_bytes.len() as i32)
                .is_err()
        {
            return WASI_EINVAL;
        }

        let ft = dir_entry
            .file_type()
            .map(|ft| {
                if ft.is_dir() {
                    WASI_FILETYPE_DIRECTORY
                } else if ft.is_symlink() {
                    WASI_FILETYPE_SYMBOLIC_LINK
                } else {
                    WASI_FILETYPE_REGULAR_FILE
                }
            })
            .unwrap_or(WASI_FILETYPE_UNKNOWN);

        if memory.write_u8(base + 20, ft).is_err()
            || memory.write_bytes(base + 24, name_bytes).is_err()
        {
            return WASI_EINVAL;
        }

        offset += entry_size;
    }

    if memory
        .write_i32(bufused_ptr as usize, offset as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

// ── args & environ ────────────────────────────────────────────────────

pub fn args_sizes_get(
    count_ptr: u32,
    buf_size_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let (argc, buf_size) = if let Ok(e) = env.lock() {
        let args = e.args();
        let total: usize = args.iter().map(|a| a.len() + 1).sum();
        (args.len() as i32, total as i32)
    } else {
        (0, 0)
    };

    if memory.write_i32(count_ptr as usize, argc).is_err()
        || memory.write_i32(buf_size_ptr as usize, buf_size).is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn args_get(
    argv_ptr: u32,
    argv_buf_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let args: Vec<String> = if let Ok(e) = env.lock() {
        e.args().to_vec()
    } else {
        return WASI_EIO;
    };

    let mut buf_offset = argv_buf_ptr as usize;
    for (i, arg) in args.iter().enumerate() {
        let ptr_addr = argv_ptr as usize + i * 4;
        if memory.write_i32(ptr_addr, buf_offset as i32).is_err()
            || memory.write_bytes(buf_offset, arg.as_bytes()).is_err()
        {
            return WASI_EINVAL;
        }
        buf_offset += arg.len();
        if memory.write_u8(buf_offset, 0).is_err() {
            return WASI_EINVAL;
        }
        buf_offset += 1;
    }
    WASI_ESUCCESS
}

pub fn environ_sizes_get(
    count_ptr: u32,
    buf_size_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let (count, buf_size) = if let Ok(e) = env.lock() {
        let vars = e.env_vars();
        let total: usize = vars.iter().map(|(k, v)| k.len() + 1 + v.len() + 1).sum();
        (vars.len() as i32, total as i32)
    } else {
        (0, 0)
    };

    if memory.write_i32(count_ptr as usize, count).is_err()
        || memory.write_i32(buf_size_ptr as usize, buf_size).is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn environ_get(
    environ_ptr: u32,
    environ_buf_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let vars: Vec<(String, String)> = if let Ok(e) = env.lock() {
        e.env_vars().to_vec()
    } else {
        return WASI_EIO;
    };

    let mut buf_offset = environ_buf_ptr as usize;
    for (i, (key, value)) in vars.iter().enumerate() {
        let ptr_addr = environ_ptr as usize + i * 4;
        let entry = format!("{key}={value}");
        if memory.write_i32(ptr_addr, buf_offset as i32).is_err()
            || memory.write_bytes(buf_offset, entry.as_bytes()).is_err()
        {
            return WASI_EINVAL;
        }
        buf_offset += entry.len();
        if memory.write_u8(buf_offset, 0).is_err() {
            return WASI_EINVAL;
        }
        buf_offset += 1;
    }
    WASI_ESUCCESS
}

// ── clock & random ────────────────────────────────────────────────────

pub fn clock_time_get(
    clock_id: u32,
    _precision: i64,
    time_ptr: u32,
    memory: &mut LinearMemory,
) -> i32 {
    let nanos: i64 = match clock_id {
        WASI_CLOCK_REALTIME | WASI_CLOCK_MONOTONIC => {
            match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(d) => d.as_nanos() as i64,
                Err(_) => return WASI_EIO,
            }
        }
        _ => return WASI_EINVAL,
    };

    if memory.write_i64(time_ptr as usize, nanos).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

pub fn random_get(buf_ptr: u32, buf_len: u32, memory: &mut LinearMemory) -> i32 {
    let seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => 0x12345678,
    };
    let mut state = seed;
    for i in 0..buf_len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if memory
            .write_u8(buf_ptr as usize + i as usize, (state & 0xFF) as u8)
            .is_err()
        {
            return WASI_EINVAL;
        }
    }
    WASI_ESUCCESS
}

// ── filestat helper ───────────────────────────────────────────────────

fn write_filestat(buf_ptr: u32, metadata: &std::fs::Metadata, memory: &mut LinearMemory) -> i32 {
    // filestat layout (64 bytes):
    //   0: dev     8: ino    16: filetype (u8)  24: nlink
    //  32: size   40: atim   48: mtim           56: ctim
    let base = buf_ptr as usize;
    for i in 0..64 {
        if memory.write_u8(base + i, 0).is_err() {
            return WASI_EINVAL;
        }
    }

    let filetype = if metadata.is_dir() {
        WASI_FILETYPE_DIRECTORY
    } else if metadata.is_file() {
        WASI_FILETYPE_REGULAR_FILE
    } else if metadata.file_type().is_symlink() {
        WASI_FILETYPE_SYMBOLIC_LINK
    } else {
        WASI_FILETYPE_UNKNOWN
    };

    let to_nanos = |t: std::io::Result<SystemTime>| -> i64 {
        t.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    };

    if memory.write_u8(base + 16, filetype).is_err()
        || memory.write_i64(base + 24, 1).is_err()
        || memory.write_i64(base + 32, metadata.len() as i64).is_err()
        || memory
            .write_i64(base + 40, to_nanos(metadata.accessed()))
            .is_err()
        || memory
            .write_i64(base + 48, to_nanos(metadata.modified()))
            .is_err()
        || memory
            .write_i64(base + 56, to_nanos(metadata.created()))
            .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

// ── tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_env() -> Arc<Mutex<WasiEnv>> {
        Arc::new(Mutex::new(WasiEnv::new()))
    }

    fn make_env_with_args(args: Vec<String>) -> Arc<Mutex<WasiEnv>> {
        Arc::new(Mutex::new(WasiEnv::new().with_args(args)))
    }

    #[test]
    fn test_fd_write_stdout_single_iovec() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"Hello").unwrap();
        mem.write_i32(0, 100).unwrap();
        mem.write_i32(4, 5).unwrap();
        let errno = fd_write(WASI_STDOUT_FD, 0, 1, 16, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(16).unwrap(), 5);
        assert_eq!(env.lock().unwrap().get_stdout(), b"Hello");
    }

    #[test]
    fn test_fd_write_stderr() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"error msg").unwrap();
        mem.write_i32(0, 100).unwrap();
        mem.write_i32(4, 9).unwrap();
        let errno = fd_write(WASI_STDERR_FD, 0, 1, 16, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(env.lock().unwrap().get_stderr(), b"error msg");
    }

    #[test]
    fn test_fd_write_multiple_iovecs() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(200, b"Hello, ").unwrap();
        mem.write_bytes(300, b"World!").unwrap();
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 7).unwrap();
        mem.write_i32(8, 300).unwrap();
        mem.write_i32(12, 6).unwrap();
        let errno = fd_write(WASI_STDOUT_FD, 0, 2, 32, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(32).unwrap(), 13);
        assert_eq!(env.lock().unwrap().get_stdout(), b"Hello, World!");
    }

    #[test]
    fn test_fd_write_bad_fd() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_i32(0, 100).unwrap();
        mem.write_i32(4, 1).unwrap();
        mem.write_u8(100, b'x').unwrap();
        let errno = fd_write(99, 0, 1, 16, &mut mem, &env);
        assert_eq!(errno, WASI_EBADF);
    }

    #[test]
    fn test_fd_read_stdin_returns_zero() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = fd_read(WASI_STDIN_FD, 0, 0, 100, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(100).unwrap(), 0);
    }

    #[test]
    fn test_fd_fdstat_get_stdout() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = fd_fdstat_get(WASI_STDOUT_FD, 0, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_u8(0).unwrap(), WASI_FILETYPE_CHARACTER_DEVICE);
    }

    #[test]
    fn test_fd_prestat_get_no_preopens() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(fd_prestat_get(3, 0, &mut mem, &env), WASI_EBADF);
        assert_eq!(fd_prestat_get(4, 0, &mut mem, &env), WASI_EBADF);
    }

    #[test]
    fn test_fd_prestat_get_with_preopen() {
        let tmp = std::env::temp_dir();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/test", &tmp)));
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = fd_prestat_get(3, 100, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_u8(100).unwrap(), 0); // __WASI_PREOPENTYPE_DIR
        assert_eq!(mem.read_i32(104).unwrap(), 5); // "/test" = 5 bytes
    }

    #[test]
    fn test_fd_prestat_dir_name_with_preopen() {
        let tmp = std::env::temp_dir();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/test", &tmp)));
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = fd_prestat_dir_name(3, 200, 5, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let name = mem.read_bytes(200, 5).unwrap();
        assert_eq!(&name, b"/test");
    }

    #[test]
    fn test_args_sizes_get_empty() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = args_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 0);
        assert_eq!(mem.read_i32(4).unwrap(), 0);
    }

    #[test]
    fn test_args_roundtrip() {
        let env = make_env_with_args(vec!["prog".into(), "hello".into()]);
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = args_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 2);
        assert_eq!(mem.read_i32(4).unwrap(), 11);
        let errno = args_get(100, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let ptr0 = mem.read_i32(100).unwrap() as usize;
        let ptr1 = mem.read_i32(104).unwrap() as usize;
        assert_eq!(&mem.read_bytes(ptr0, 4).unwrap(), b"prog");
        assert_eq!(mem.read_u8(ptr0 + 4).unwrap(), 0);
        assert_eq!(&mem.read_bytes(ptr1, 5).unwrap(), b"hello");
    }

    #[test]
    fn test_environ_roundtrip() {
        let env = Arc::new(Mutex::new(
            WasiEnv::new()
                .with_env("FOO".into(), "bar".into())
                .with_env("A".into(), "1".into()),
        ));
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = environ_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 2);
        let errno = environ_get(100, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let ptr0 = mem.read_i32(100).unwrap() as usize;
        assert_eq!(&mem.read_bytes(ptr0, 7).unwrap(), b"FOO=bar");
    }

    #[test]
    fn test_clock_time_get_realtime() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = clock_time_get(WASI_CLOCK_REALTIME, 0, 0, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        assert!(mem.read_i64(0).unwrap() > 0);
    }

    #[test]
    fn test_clock_time_get_monotonic() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = clock_time_get(WASI_CLOCK_MONOTONIC, 0, 0, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        assert!(mem.read_i64(0).unwrap() > 0);
    }

    #[test]
    fn test_clock_time_get_invalid_clock() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(clock_time_get(99, 0, 0, &mut mem), WASI_EINVAL);
    }

    #[test]
    fn test_random_get_fills_buffer() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = random_get(0, 16, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        let bytes = mem.read_bytes(0, 16).unwrap();
        assert!(bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_random_get_zero_length() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(random_get(0, 0, &mut mem), WASI_ESUCCESS);
    }

    #[test]
    fn test_args_sizes_get_with_args() {
        let env = make_env_with_args(vec!["a".into(), "bb".into(), "ccc".into()]);
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = args_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 3);
        assert_eq!(mem.read_i32(4).unwrap(), 9);
    }

    #[test]
    fn test_environ_sizes_get_empty() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = environ_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 0);
        assert_eq!(mem.read_i32(4).unwrap(), 0);
    }

    #[test]
    fn test_fd_close_stdio() {
        let env = make_env();
        assert_eq!(fd_close(WASI_STDOUT_FD, &env), WASI_ESUCCESS);
    }

    #[test]
    fn test_fd_close_unknown() {
        let env = make_env();
        assert_eq!(fd_close(99, &env), WASI_EBADF);
    }

    #[test]
    fn test_path_open_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"nope.txt").unwrap();
        assert_eq!(path_open(3, 100, 8, 0, 0, 200, &mut mem, &env), WASI_ENOENT);
    }

    #[test]
    fn test_path_unlink_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"nope.txt").unwrap();
        assert_eq!(path_unlink_file(3, 100, 8, &mut mem, &env), WASI_ENOENT);
    }

    #[test]
    fn test_path_open_create_and_read_file() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Create file
        mem.write_bytes(100, b"test.txt").unwrap();
        let errno = path_open(3, 100, 8, WASI_O_CREAT, 0, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert!(tmp.path().join("test.txt").exists());

        let fd = mem.read_i32(200).unwrap() as u32;

        // Write via host fs for testing read
        std::fs::write(tmp.path().join("test.txt"), b"hello").unwrap();

        // Read back
        mem.write_i32(0, 400).unwrap(); // iovec buf_ptr
        mem.write_i32(4, 100).unwrap(); // iovec buf_len
        let errno = fd_read(fd, 0, 1, 300, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(300).unwrap(), 5);
        assert_eq!(&mem.read_bytes(400, 5).unwrap(), b"hello");
    }

    #[test]
    fn test_fd_seek_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("data.txt"), b"0123456789").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Open file
        mem.write_bytes(100, b"data.txt").unwrap();
        path_open(3, 100, 8, 0, 0, 200, &mut mem, &env);
        let fd = mem.read_i32(200).unwrap() as u32;

        // Seek to offset 5
        let errno = fd_seek(fd, 5, WASI_WHENCE_SET, 300, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i64(300).unwrap(), 5);

        // Read from offset 5
        mem.write_i32(0, 400).unwrap();
        mem.write_i32(4, 10).unwrap();
        let errno = fd_read(fd, 0, 1, 500, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(500).unwrap(), 5); // "56789"
        assert_eq!(&mem.read_bytes(400, 5).unwrap(), b"56789");
    }
}
