//! WASI syscall implementations that operate on linear memory.

use crate::runtime::core::memory::LinearMemory;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{FdKind, SocketHandle, WasiEnv, WASI_STDERR_FD, WASI_STDIN_FD, WASI_STDOUT_FD};

pub const WASI_ESUCCESS: i32 = 0;
pub const WASI_EBADF: i32 = 8;
pub const WASI_EDQUOT: i32 = 19;
pub const WASI_EEXIST: i32 = 20;
pub const WASI_EFBIG: i32 = 22;
pub const WASI_EINTR: i32 = 27;
pub const WASI_EINVAL: i32 = 28;
pub const WASI_EIO: i32 = 29;
pub const WASI_EISDIR: i32 = 31;
pub const WASI_ENOENT: i32 = 44;
pub const WASI_ENOSYS: i32 = 52;
pub const WASI_ENOTDIR: i32 = 54;
pub const WASI_ENOTEMPTY: i32 = 55;
pub const WASI_ERANGE: i32 = 68;
pub const WASI_EACCES: i32 = 2;
pub const WASI_EAGAIN: i32 = 6;
pub const WASI_ECONNABORTED: i32 = 13;
pub const WASI_ENOTSOCK: i32 = 57;
pub const WASI_EPIPE: i32 = 64;

/// `fstflags`: which of a file's timestamps `path_filestat_set_times` writes.
const WASI_FILESTAT_SET_ATIM: u16 = 1;
const WASI_FILESTAT_SET_ATIM_NOW: u16 = 2;
const WASI_FILESTAT_SET_MTIM: u16 = 4;
const WASI_FILESTAT_SET_MTIM_NOW: u16 = 8;

pub const WASI_CLOCK_REALTIME: u32 = 0;
pub const WASI_CLOCK_MONOTONIC: u32 = 1;

pub const WASI_FILETYPE_UNKNOWN: u8 = 0;
pub const WASI_FILETYPE_CHARACTER_DEVICE: u8 = 2;
pub const WASI_FILETYPE_DIRECTORY: u8 = 3;
pub const WASI_FILETYPE_REGULAR_FILE: u8 = 4;
pub const WASI_FILETYPE_SOCKET_STREAM: u8 = 6;
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
    if is_socket_fd(fd, env) {
        return socket_write_iovs(fd, iovs_ptr, iovs_len, nwritten_ptr, memory, env);
    }

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
                    e.write_stdout(&bytes);
                }
            }
            WASI_STDERR_FD => {
                // Pass stderr through immediately so error messages are visible
                eprint!("{}", String::from_utf8_lossy(&bytes));
                if let Ok(mut e) = env.lock() {
                    e.write_stderr(&bytes);
                }
            }
            _ => {
                let mut e = match env.lock() {
                    Ok(e) => e,
                    Err(_) => return WASI_EIO,
                };
                let max_file_size = e.max_file_size();
                let max_disk = e.max_disk_bytes();
                let (host_path, offset) = match e.get_fd(fd) {
                    Some(entry) if entry.kind == FdKind::File => {
                        (entry.host_path.clone(), entry.offset)
                    }
                    Some(_) => return WASI_EISDIR,
                    None => return WASI_EBADF,
                };
                // Enforce the per-file and total-disk caps. The file's resulting
                // size is the larger of its current size and the end of this
                // write; the disk delta is just the growth beyond the old size.
                let mut disk_delta: u64 = 0;
                if max_file_size.is_some() || max_disk.is_some() {
                    let existing = std::fs::metadata(&host_path).map(|m| m.len()).unwrap_or(0);
                    let projected = existing.max(offset + bytes.len() as u64);
                    if let Some(max) = max_file_size {
                        if projected > max {
                            return WASI_EFBIG;
                        }
                    }
                    disk_delta = projected.saturating_sub(existing);
                    if let Some(max) = max_disk {
                        if e.disk_used().saturating_add(disk_delta) > max {
                            return WASI_EDQUOT;
                        }
                    }
                }
                match write_file_at(&host_path, offset, &bytes) {
                    Ok(n) => {
                        if let Some(fe) = e.get_fd_mut(fd) {
                            fe.offset += n as u64;
                        }
                        if max_disk.is_some() {
                            e.add_disk_used(disk_delta);
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

/// Fill the iovecs from the session's stdin buffer.
///
/// Whatever is left is handed over in order across calls, and a read past the
/// end reports 0 bytes: WASI has no way to block here, so an exhausted (or
/// never-supplied) stdin is simply EOF.
fn read_stdin_into(
    iovs_ptr: u32,
    iovs_len: u32,
    nread_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let mut e = match env.lock() {
        Ok(e) => e,
        Err(_) => return WASI_EIO,
    };

    let mut total_read: u32 = 0;
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
        let chunk = e.read_stdin(buf_len);
        if chunk.is_empty() {
            break;
        }
        if memory.write_bytes(buf_ptr, &chunk).is_err() {
            return WASI_EINVAL;
        }
        total_read += chunk.len() as u32;
    }

    if memory
        .write_i32(nread_ptr as usize, total_read as i32)
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
        return read_stdin_into(iovs_ptr, iovs_len, nread_ptr, memory, env);
    }
    if fd == WASI_STDOUT_FD || fd == WASI_STDERR_FD {
        return WASI_EBADF;
    }
    // A stock `wasm32-wasip1` guest reads an accepted connection with the
    // ordinary file calls, not with `sock_recv`, so this is the path that
    // actually carries socket traffic.
    if is_socket_fd(fd, env) {
        return socket_read_iovs(fd, iovs_ptr, iovs_len, nread_ptr, memory, env);
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
        FdKind::SocketListener | FdKind::SocketStream => {
            (WASI_FILETYPE_SOCKET_STREAM, 0u16, 0x0FFF_FFFFu64)
        }
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

    if trunc && host_path.is_file() {
        // Truncation empties the file, so its bytes are returned to the quota.
        let freed = std::fs::metadata(&host_path).map(|m| m.len()).unwrap_or(0);
        if std::fs::File::create(&host_path).is_err() {
            return WASI_EIO;
        }
        if e.max_disk_bytes().is_some() {
            e.sub_disk_used(freed);
        }
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

    let mut e = match env.lock() {
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

    // Capture the size first so the freed bytes can be returned to the quota.
    let freed = std::fs::metadata(&host_path).map(|m| m.len()).unwrap_or(0);
    match std::fs::remove_file(&host_path) {
        Ok(_) => {
            if e.max_disk_bytes().is_some() {
                e.sub_disk_used(freed);
            }
            WASI_ESUCCESS
        }
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

/// fd_fdstat_set_flags: set file descriptor flags (WASI Preview 1).
/// Most WASM runtimes ignore this silently; return success.
pub fn fd_fdstat_set_flags(_fd: u32, _flags: u16) -> i32 {
    WASI_ESUCCESS
}

/// path_filestat_set_times: set file timestamps. Return ENOSYS since we
/// don't have a mutable host FS. Callers treat ENOSYS as non-fatal.
/// path_filestat_set_times: set a file's access and modification times.
///
/// `fst_flags` picks which of the two to set and whether to take the value from
/// the corresponding argument or from the current clock. A timestamp that is
/// not being set has to be left at whatever the file already carries, which is
/// why the existing metadata is read first.
#[allow(clippy::too_many_arguments)]
pub fn path_filestat_set_times(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    atim: u64,
    mtim: u64,
    fst_flags: u16,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let set_atim = fst_flags & WASI_FILESTAT_SET_ATIM != 0;
    let set_atim_now = fst_flags & WASI_FILESTAT_SET_ATIM_NOW != 0;
    let set_mtim = fst_flags & WASI_FILESTAT_SET_MTIM != 0;
    let set_mtim_now = fst_flags & WASI_FILESTAT_SET_MTIM_NOW != 0;

    // Asking for both an explicit timestamp and "now" for the same field is
    // contradictory, and the spec says to reject it rather than pick one.
    if (set_atim && set_atim_now) || (set_mtim && set_mtim_now) {
        return WASI_EINVAL;
    }

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

    let metadata = match std::fs::metadata(&host_path) {
        Ok(m) => m,
        Err(_) => return WASI_ENOENT,
    };

    let now = SystemTime::now();
    let atime = if set_atim {
        UNIX_EPOCH + Duration::from_nanos(atim)
    } else if set_atim_now {
        now
    } else {
        metadata.accessed().unwrap_or(now)
    };
    let mtime = if set_mtim {
        UNIX_EPOCH + Duration::from_nanos(mtim)
    } else if set_mtim_now {
        now
    } else {
        metadata.modified().unwrap_or(now)
    };

    let file = match std::fs::OpenOptions::new().write(true).open(&host_path) {
        Ok(f) => f,
        Err(err) => return errno_from_io(&err),
    };
    let times = std::fs::FileTimes::new()
        .set_accessed(atime)
        .set_modified(mtime);
    match file.set_times(times) {
        Ok(()) => WASI_ESUCCESS,
        Err(err) => errno_from_io(&err),
    }
}

/// path_readlink: read a symbolic link's target into `buf`.
///
/// The target is written untruncated or not at all: WASI has no way to say
/// "here is a prefix", so a buffer that is too small is an ERANGE rather than a
/// short write the guest would mistake for the whole path.
#[allow(clippy::too_many_arguments)]
pub fn path_readlink(
    dir_fd: u32,
    path_ptr: u32,
    path_len: u32,
    buf_ptr: u32,
    buf_len: u32,
    buf_used_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let _ = memory.write_i32(buf_used_ptr as usize, 0);

    let path = match read_guest_string(path_ptr, path_len, memory) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let host_path = {
        let e = match env.lock() {
            Ok(e) => e,
            Err(_) => return WASI_EIO,
        };
        match e.resolve_path(dir_fd, &path) {
            Ok(p) => p,
            Err(_) => return WASI_EBADF,
        }
    };

    let target = match std::fs::read_link(&host_path) {
        Ok(t) => t,
        Err(err) => return errno_from_io(&err),
    };

    let bytes = target.to_string_lossy().into_owned().into_bytes();
    if bytes.len() > buf_len as usize {
        return WASI_ERANGE;
    }
    if memory.write_bytes(buf_ptr as usize, &bytes).is_err() {
        return WASI_EINVAL;
    }
    if memory
        .write_i32(buf_used_ptr as usize, bytes.len() as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// Map an I/O error onto the closest WASI errno.
fn errno_from_io(err: &std::io::Error) -> i32 {
    match err.kind() {
        std::io::ErrorKind::NotFound => WASI_ENOENT,
        std::io::ErrorKind::PermissionDenied => WASI_EACCES,
        std::io::ErrorKind::AlreadyExists => WASI_EEXIST,
        std::io::ErrorKind::InvalidInput => WASI_EINVAL,
        // read_link on something that is not a symlink reports this on every
        // platform wasmrun builds for.
        _ => WASI_EINVAL,
    }
}

/// `eventtype`: what a subscription is waiting on.
const WASI_EVENTTYPE_CLOCK: u8 = 0;
const WASI_EVENTTYPE_FD_READ: u8 = 1;
const WASI_EVENTTYPE_FD_WRITE: u8 = 2;

/// `subclockflags`: set when a clock subscription's timeout is an absolute
/// point in time rather than a duration from now.
const WASI_SUBSCRIPTION_CLOCK_ABSTIME: u16 = 1;

/// Layout of `subscription` and `event`, which the guest lays out for us.
const SUBSCRIPTION_SIZE: u32 = 48;
const EVENT_SIZE: u32 = 32;

/// How long a single sleep runs before the cancellation flag is re-checked.
const POLL_SLICE: Duration = Duration::from_millis(20);

/// poll_oneoff: wait until at least one of `nsubscriptions` subscriptions is
/// ready, then write one event per ready subscription.
///
/// The interpreter is single-threaded and every file it can reach is a regular
/// file, so the two halves of this behave very differently. An `fd_read` or
/// `fd_write` subscription is ready the moment it is asked about, which is what
/// POSIX says about regular files and is why `select` over them is not a wait
/// at all. A `clock` subscription is a real wait, and is the reason programs
/// call this at all: it is what `thread::sleep` and every timer lowers to.
///
/// When both kinds are present the ready file descriptors win and no sleeping
/// happens, which is the same answer a real poll would give.
pub fn poll_oneoff(
    in_ptr: u32,
    out_ptr: u32,
    nsubscriptions: u32,
    nevents_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let _ = memory.write_i32(nevents_ptr as usize, 0);

    // Waiting on nothing would block forever, so the spec makes it an error.
    if nsubscriptions == 0 {
        return WASI_EINVAL;
    }

    let mut subscriptions = Vec::with_capacity(nsubscriptions as usize);
    for i in 0..nsubscriptions {
        let base = in_ptr + i * SUBSCRIPTION_SIZE;
        let userdata = match memory.read_i64(base as usize) {
            Ok(v) => v as u64,
            Err(_) => return WASI_EINVAL,
        };
        let tag = match memory.read_u8((base + 8) as usize) {
            Ok(v) => v,
            Err(_) => return WASI_EINVAL,
        };
        subscriptions.push((userdata, tag, base));
    }

    let cancel = env.lock().ok().and_then(|e| e.cancel_token().cloned());

    // Pass one: anything ready without waiting.
    let mut events: Vec<(u64, u8, i32, u64)> = Vec::new();
    for &(userdata, tag, base) in &subscriptions {
        match tag {
            WASI_EVENTTYPE_FD_READ | WASI_EVENTTYPE_FD_WRITE => {
                let fd = match memory.read_i32((base + 16) as usize) {
                    Ok(v) => v as u32,
                    Err(_) => return WASI_EINVAL,
                };
                let (errno, nbytes) = poll_fd_readiness(fd, tag, env);
                events.push((userdata, tag, errno, nbytes));
            }
            WASI_EVENTTYPE_CLOCK => {}
            _ => events.push((userdata, tag, WASI_EINVAL, 0)),
        }
    }

    if !events.is_empty() {
        return write_events(out_ptr, nevents_ptr, &events, memory);
    }

    // Pass two: every subscription is a clock, so this is a real wait. Sleep
    // until the earliest deadline, then report every clock that has come due.
    let now_realtime = wall_clock_nanos();
    let start = Instant::now();
    let mut shortest: Option<Duration> = None;
    let mut deadlines: Vec<(u64, Option<Duration>, i32)> = Vec::new();

    for &(userdata, _tag, base) in &subscriptions {
        let clock_id = match memory.read_i32((base + 16) as usize) {
            Ok(v) => v as u32,
            Err(_) => return WASI_EINVAL,
        };
        let timeout = match memory.read_i64((base + 24) as usize) {
            Ok(v) => v as u64,
            Err(_) => return WASI_EINVAL,
        };
        let flags = match memory.read_u16((base + 40) as usize) {
            Ok(v) => v,
            Err(_) => return WASI_EINVAL,
        };

        if clock_id != WASI_CLOCK_REALTIME && clock_id != WASI_CLOCK_MONOTONIC {
            deadlines.push((userdata, None, WASI_EINVAL));
            continue;
        }

        let wait = if flags & WASI_SUBSCRIPTION_CLOCK_ABSTIME != 0 {
            // An absolute realtime deadline is measured against the wall clock;
            // an absolute monotonic one against the same monotonic origin the
            // guest read, which for us is also nanoseconds since the epoch.
            Duration::from_nanos(timeout.saturating_sub(now_realtime))
        } else {
            Duration::from_nanos(timeout)
        };
        shortest = Some(match shortest {
            Some(s) if s <= wait => s,
            _ => wait,
        });
        deadlines.push((userdata, Some(wait), WASI_ESUCCESS));
    }

    if let Some(wait) = shortest {
        // Sleep in slices so a cancelled execution stops here rather than
        // running out the whole timeout inside a host call.
        while start.elapsed() < wait {
            if let Some(flag) = cancel.as_ref() {
                if flag.load(Ordering::Relaxed) {
                    return WASI_EINTR;
                }
            }
            let remaining = wait - start.elapsed();
            std::thread::sleep(remaining.min(POLL_SLICE));
        }
    }

    let elapsed = start.elapsed();
    let mut clock_events: Vec<(u64, u8, i32, u64)> = Vec::new();
    for (userdata, wait, errno) in deadlines {
        match wait {
            // An invalid clock id is reported against that subscription alone,
            // rather than failing the whole call and stranding the valid ones.
            None => clock_events.push((userdata, WASI_EVENTTYPE_CLOCK, errno, 0)),
            Some(w) if w <= elapsed => {
                clock_events.push((userdata, WASI_EVENTTYPE_CLOCK, WASI_ESUCCESS, 0))
            }
            Some(_) => {}
        }
    }

    write_events(out_ptr, nevents_ptr, &clock_events, memory)
}

/// Whether a descriptor can be read from or written to right now, and how many
/// bytes are available if it can.
fn poll_fd_readiness(fd: u32, tag: u8, env: &Arc<Mutex<WasiEnv>>) -> (i32, u64) {
    let mut e = match env.lock() {
        Ok(e) => e,
        Err(_) => return (WASI_EIO, 0),
    };
    let entry = match e.get_fd(fd) {
        Some(entry) => entry,
        None => return (WASI_EBADF, 0),
    };
    match (entry.kind, tag) {
        // stdin reports what is left of the input it was given, so a program
        // polling before reading learns whether a read would return anything.
        (FdKind::Stdin, WASI_EVENTTYPE_FD_READ) => (WASI_ESUCCESS, e.stdin_remaining() as u64),
        (FdKind::Stdin, _) => (WASI_EBADF, 0),
        (FdKind::Stdout | FdKind::Stderr, WASI_EVENTTYPE_FD_WRITE) => (WASI_ESUCCESS, 0),
        (FdKind::Stdout | FdKind::Stderr, _) => (WASI_EBADF, 0),
        // A regular file is always ready both ways.
        (FdKind::File, WASI_EVENTTYPE_FD_READ) => {
            let remaining = std::fs::metadata(&entry.host_path)
                .map(|m| m.len().saturating_sub(entry.offset))
                .unwrap_or(0);
            (WASI_ESUCCESS, remaining)
        }
        (FdKind::File, _) => (WASI_ESUCCESS, 0),
        (FdKind::PreopenDir | FdKind::Directory, _) => (WASI_EBADF, 0),
        // A socket is ready when the kernel says so, which is the point of
        // polling one: reporting it always-ready the way a file is would turn
        // a poll loop into a spin. `peek` on a non-blocking socket answers
        // without consuming anything.
        (FdKind::SocketStream, WASI_EVENTTYPE_FD_READ) => match e.socket(fd) {
            Some(SocketHandle::Stream(stream)) => {
                let mut probe = [0u8; 1];
                match stream.peek(&mut probe) {
                    // Zero bytes peeked means the peer is gone, so a read
                    // would return EOF immediately: ready, with nothing on it.
                    Ok(n) => (WASI_ESUCCESS, n as u64),
                    Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        (WASI_EAGAIN, 0)
                    }
                    Err(_) => (WASI_EIO, 0),
                }
            }
            _ => (WASI_EBADF, 0),
        },
        // Writability is not predicted: a send that would block reports it
        // itself rather than this claiming to know in advance.
        (FdKind::SocketStream, _) => (WASI_ESUCCESS, 0),
        // A listener is readable when a connection is waiting. There is no way
        // to ask without taking it, so a probe that finds one parks it for the
        // `sock_accept` that follows rather than dropping the connection.
        (FdKind::SocketListener, WASI_EVENTTYPE_FD_READ) => {
            if e.has_pending_accept(fd) {
                (WASI_ESUCCESS, 1)
            } else {
                match e.socket(fd) {
                    Some(SocketHandle::Listener(listener)) => match listener.accept() {
                        Ok((stream, _)) => {
                            e.push_pending_accept(fd, stream);
                            (WASI_ESUCCESS, 1)
                        }
                        Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            (WASI_EAGAIN, 0)
                        }
                        Err(_) => (WASI_EIO, 0),
                    },
                    _ => (WASI_EBADF, 0),
                }
            }
        }
        (FdKind::SocketListener, _) => (WASI_EBADF, 0),
    }
}

/// Write `events` into the guest's output array and record how many there are.
fn write_events(
    out_ptr: u32,
    nevents_ptr: u32,
    events: &[(u64, u8, i32, u64)],
    memory: &mut LinearMemory,
) -> i32 {
    for (i, (userdata, tag, errno, nbytes)) in events.iter().enumerate() {
        let base = out_ptr as usize + i * EVENT_SIZE as usize;
        if memory.write_i64(base, *userdata as i64).is_err()
            || memory.write_u16(base + 8, *errno as u16).is_err()
            || memory.write_u8(base + 10, *tag).is_err()
            || memory.write_i64(base + 16, *nbytes as i64).is_err()
            || memory.write_u16(base + 24, 0).is_err()
        {
            return WASI_EINVAL;
        }
    }
    if memory
        .write_i32(nevents_ptr as usize, events.len() as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// Nanoseconds since the Unix epoch, which is the origin both clocks report.
fn wall_clock_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// sched_yield: give up the rest of this time slice.
///
/// There is nothing else in the sandbox to yield to, so this only ever hands
/// the hint to the host scheduler and reports success.
pub fn sched_yield() -> i32 {
    std::thread::yield_now();
    WASI_ESUCCESS
}

// ── Sockets ──────────────────────────────────────────────
//
// Preview 1 gives a guest no way to create a socket, so everything here starts
// from a listener the host bound and preopened. `sock_accept` turns that into
// a connected fd, and the guest then reads and writes it with `fd_read` and
// `fd_write`, which is what a stock `wasm32-wasip1` Rust binary does: its
// `TcpListener::accept` lowers to `sock_accept` and its `TcpStream` to the
// ordinary file calls. `sock_recv` and `sock_send` are the same operations
// under their socket names, for guests whose libc uses them.

/// How long a blocking socket call waits before re-checking cancellation.
const SOCKET_SLICE: Duration = Duration::from_millis(20);

/// Run `attempt` until it returns something other than `WouldBlock`, giving up
/// if the execution is cancelled.
///
/// The executor can only notice cancellation between instructions, which never
/// happens while a host call is blocked, so every wait in this file is sliced
/// the way `poll_oneoff` slices its sleep.
fn wait_for_socket<T>(
    env: &Arc<Mutex<WasiEnv>>,
    mut attempt: impl FnMut() -> std::io::Result<T>,
) -> std::result::Result<T, i32> {
    let cancel = env.lock().ok().and_then(|e| e.cancel_token().cloned());
    loop {
        match attempt() {
            Ok(value) => return Ok(value),
            Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(ref err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(io_errno(&err)),
        }

        if let Some(flag) = cancel.as_ref() {
            if flag.load(Ordering::Relaxed) {
                return Err(WASI_EINTR);
            }
        }
        std::thread::sleep(SOCKET_SLICE);
    }
}

fn io_errno(err: &std::io::Error) -> i32 {
    match err.kind() {
        std::io::ErrorKind::WouldBlock => WASI_EAGAIN,
        std::io::ErrorKind::BrokenPipe => WASI_EPIPE,
        std::io::ErrorKind::ConnectionAborted => WASI_ECONNABORTED,
        std::io::ErrorKind::ConnectionReset => WASI_ECONNABORTED,
        std::io::ErrorKind::InvalidInput => WASI_EINVAL,
        _ => WASI_EIO,
    }
}

/// Whether an fd names a connected socket, which decides who serves `fd_read`
/// and `fd_write` for it.
fn is_socket_fd(fd: u32, env: &Arc<Mutex<WasiEnv>>) -> bool {
    env.lock()
        .ok()
        .and_then(|e| e.get_fd(fd).map(|entry| entry.kind == FdKind::SocketStream))
        .unwrap_or(false)
}

/// The stream behind an fd, or the errno explaining why there is not one.
fn stream_for(fd: u32, env: &Arc<Mutex<WasiEnv>>) -> std::result::Result<Arc<TcpStream>, i32> {
    let e = env.lock().map_err(|_| WASI_EIO)?;
    match e.get_fd(fd) {
        Some(entry) if entry.kind == FdKind::SocketStream => match e.socket(fd) {
            Some(SocketHandle::Stream(stream)) => Ok(stream),
            _ => Err(WASI_EBADF),
        },
        Some(_) => Err(WASI_ENOTSOCK),
        None => Err(WASI_EBADF),
    }
}

/// sock_accept: take the next connection on a listening fd.
///
/// `flags` carries `fdflags`, of which only `nonblock` (bit 0) means anything
/// here: with it set the call reports `EAGAIN` rather than waiting.
pub fn sock_accept(
    fd: u32,
    flags: u32,
    result_fd_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    const FDFLAG_NONBLOCK: u32 = 0x0004;
    let nonblocking = flags & FDFLAG_NONBLOCK != 0;

    let listener = {
        let mut e = match env.lock() {
            Ok(e) => e,
            Err(_) => return WASI_EIO,
        };
        // A poll may already have taken a connection to find out there was one.
        if let Some(stream) = e.take_pending_accept(fd) {
            let new_fd = e.add_socket_stream(stream);
            drop(e);
            return write_accepted_fd(new_fd, result_fd_ptr, memory);
        }
        match e.get_fd(fd) {
            Some(entry) if entry.kind == FdKind::SocketListener => match e.socket(fd) {
                Some(SocketHandle::Listener(listener)) => listener,
                _ => return WASI_EBADF,
            },
            Some(_) => return WASI_ENOTSOCK,
            None => return WASI_EBADF,
        }
    };

    let stream = if nonblocking {
        match listener.accept() {
            Ok((stream, _)) => stream,
            Err(err) => return io_errno(&err),
        }
    } else {
        match wait_for_socket(env, || listener.accept()) {
            Ok((stream, _)) => stream,
            Err(errno) => return errno,
        }
    };

    // The listener is non-blocking so accept can be sliced; the connection it
    // produces must not inherit that, or every read on it would spin.
    if stream.set_nonblocking(false).is_err() {
        return WASI_EIO;
    }

    let new_fd = match env.lock() {
        Ok(mut e) => e.add_socket_stream(stream),
        Err(_) => return WASI_EIO,
    };
    write_accepted_fd(new_fd, result_fd_ptr, memory)
}

fn write_accepted_fd(new_fd: u32, result_fd_ptr: u32, memory: &mut LinearMemory) -> i32 {
    if memory
        .write_i32(result_fd_ptr as usize, new_fd as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// Read from a connected socket into an iovec array.
///
/// Shared by `sock_recv` and by `fd_read` when the fd names a socket, since
/// they are the same operation and a guest may reach for either.
pub(crate) fn socket_read_iovs(
    fd: u32,
    iovs_ptr: u32,
    iovs_len: u32,
    nread_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let stream = match stream_for(fd, env) {
        Ok(stream) => stream,
        Err(errno) => return errno,
    };

    let mut total_read: u32 = 0;
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

        let mut buf = vec![0u8; buf_len];
        let read = match wait_for_socket(env, || (&*stream).read(&mut buf)) {
            Ok(n) => n,
            Err(errno) => return errno,
        };
        if read > 0 && memory.write_bytes(buf_ptr, &buf[..read]).is_err() {
            return WASI_EINVAL;
        }
        total_read += read as u32;
        // A short read is the whole answer: waiting to fill the next iovec
        // would block a caller that already has what it asked for.
        if read < buf_len {
            break;
        }
    }

    if memory
        .write_i32(nread_ptr as usize, total_read as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// Write an iovec array to a connected socket. Shared with `fd_write`.
pub(crate) fn socket_write_iovs(
    fd: u32,
    iovs_ptr: u32,
    iovs_len: u32,
    nwritten_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let stream = match stream_for(fd, env) {
        Ok(stream) => stream,
        Err(errno) => return errno,
    };

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

        let data = match memory.read_bytes(buf_ptr, buf_len) {
            Ok(d) => d,
            Err(_) => return WASI_EINVAL,
        };
        if let Err(errno) = wait_for_socket(env, || (&*stream).write_all(&data)) {
            return errno;
        }
        total_written += buf_len as u32;
    }

    if memory
        .write_i32(nwritten_ptr as usize, total_written as i32)
        .is_err()
    {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// sock_recv: read from a socket. `ri_flags` (peek, waitall) is not honored.
#[allow(clippy::too_many_arguments)]
pub fn sock_recv(
    fd: u32,
    ri_data_ptr: u32,
    ri_data_len: u32,
    _ri_flags: u32,
    ro_datalen_ptr: u32,
    ro_flags_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let errno = socket_read_iovs(fd, ri_data_ptr, ri_data_len, ro_datalen_ptr, memory, env);
    if errno != WASI_ESUCCESS {
        return errno;
    }
    // roflags reports out-of-band conditions; none of them apply to a stream
    // socket read that just succeeded.
    if memory.write_u16(ro_flags_ptr as usize, 0).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// sock_send: write to a socket. `si_flags` is reserved and must be zero.
pub fn sock_send(
    fd: u32,
    si_data_ptr: u32,
    si_data_len: u32,
    _si_flags: u32,
    so_datalen_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    socket_write_iovs(fd, si_data_ptr, si_data_len, so_datalen_ptr, memory, env)
}

/// sock_shutdown: close one or both directions of a connection.
pub fn sock_shutdown(fd: u32, how: u32, env: &Arc<Mutex<WasiEnv>>) -> i32 {
    const SHUT_RD: u32 = 1;
    const SHUT_WR: u32 = 2;

    let shutdown = match how {
        SHUT_RD => std::net::Shutdown::Read,
        SHUT_WR => std::net::Shutdown::Write,
        n if n == SHUT_RD | SHUT_WR => std::net::Shutdown::Both,
        _ => return WASI_EINVAL,
    };

    let stream = match stream_for(fd, env) {
        Ok(stream) => stream,
        Err(errno) => return errno,
    };
    match stream.shutdown(shutdown) {
        Ok(()) => WASI_ESUCCESS,
        // Shutting down a connection the peer already dropped is the state the
        // caller asked for, not a failure.
        Err(ref err) if err.kind() == std::io::ErrorKind::NotConnected => WASI_ESUCCESS,
        Err(err) => io_errno(&err),
    }
}

/// path_symlink: create a symbolic link. Return ENOSYS.
pub fn path_symlink() -> i32 {
    WASI_ENOSYS
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

    /// One iovec, enough room for everything: the whole input lands in memory.
    #[test]
    fn test_fd_read_stdin_fills_buffer() {
        let env = make_env();
        env.lock().unwrap().set_stdin(b"hello stdin".to_vec());
        let mut mem = LinearMemory::new(1, None).unwrap();
        // iovec at 0: buffer at 200, length 32.
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 32).unwrap();

        let errno = fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(100).unwrap(), 11);
        assert_eq!(mem.read_bytes(200, 11).unwrap(), b"hello stdin");
    }

    /// A program draining stdin in small reads gets it in order, then EOF.
    #[test]
    fn test_fd_read_stdin_resumes_across_calls() {
        let env = make_env();
        env.lock().unwrap().set_stdin(b"abcdef".to_vec());
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 4).unwrap();

        assert_eq!(
            fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(mem.read_i32(100).unwrap(), 4);
        assert_eq!(mem.read_bytes(200, 4).unwrap(), b"abcd");

        assert_eq!(
            fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(mem.read_i32(100).unwrap(), 2);
        assert_eq!(mem.read_bytes(200, 2).unwrap(), b"ef");

        // Drained: further reads are EOF, never a block.
        assert_eq!(
            fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(mem.read_i32(100).unwrap(), 0);
    }

    /// Multiple iovecs are filled in order, as a scatter read must.
    #[test]
    fn test_fd_read_stdin_across_iovecs() {
        let env = make_env();
        env.lock().unwrap().set_stdin(b"abcdefgh".to_vec());
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 3).unwrap();
        mem.write_i32(8, 300).unwrap();
        mem.write_i32(12, 16).unwrap();

        let errno = fd_read(WASI_STDIN_FD, 0, 2, 100, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(100).unwrap(), 8);
        assert_eq!(mem.read_bytes(200, 3).unwrap(), b"abc");
        assert_eq!(mem.read_bytes(300, 5).unwrap(), b"defgh");
    }

    /// Setting stdin again rewinds, so one exec never inherits another's
    /// partially-consumed input.
    #[test]
    fn test_set_stdin_rewinds() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 32).unwrap();

        env.lock().unwrap().set_stdin(b"first".to_vec());
        fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env);
        env.lock().unwrap().set_stdin(b"second".to_vec());
        fd_read(WASI_STDIN_FD, 0, 1, 100, &mut mem, &env);

        assert_eq!(mem.read_i32(100).unwrap(), 6);
        assert_eq!(mem.read_bytes(200, 6).unwrap(), b"second");
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

    // ---- WASI Preview 1 tail (0.23.2) ----

    /// Lay one subscription into guest memory at `base`.
    fn write_clock_subscription(
        mem: &mut LinearMemory,
        base: u32,
        userdata: u64,
        clock_id: u32,
        timeout_ns: u64,
        abstime: bool,
    ) {
        mem.write_i64(base as usize, userdata as i64).unwrap();
        mem.write_u8((base + 8) as usize, WASI_EVENTTYPE_CLOCK)
            .unwrap();
        mem.write_i32((base + 16) as usize, clock_id as i32)
            .unwrap();
        mem.write_i64((base + 24) as usize, timeout_ns as i64)
            .unwrap();
        mem.write_i64((base + 32) as usize, 0).unwrap();
        mem.write_u16(
            (base + 40) as usize,
            if abstime {
                WASI_SUBSCRIPTION_CLOCK_ABSTIME
            } else {
                0
            },
        )
        .unwrap();
    }

    fn write_fd_subscription(mem: &mut LinearMemory, base: u32, userdata: u64, tag: u8, fd: u32) {
        mem.write_i64(base as usize, userdata as i64).unwrap();
        mem.write_u8((base + 8) as usize, tag).unwrap();
        mem.write_i32((base + 16) as usize, fd as i32).unwrap();
    }

    /// Read back one event: (userdata, errno, type, nbytes).
    fn read_event(mem: &LinearMemory, out_ptr: u32, i: u32) -> (u64, u16, u8, u64) {
        let base = (out_ptr + i * EVENT_SIZE) as usize;
        (
            mem.read_i64(base).unwrap() as u64,
            mem.read_u16(base + 8).unwrap(),
            mem.read_u8(base + 10).unwrap(),
            mem.read_i64(base + 16).unwrap() as u64,
        )
    }

    #[test]
    fn test_efbig_is_the_preview1_value() {
        // Preview 1 numbers `fbig` 22; 27 is `intr`, which is what the constant
        // used to be set to.
        assert_eq!(WASI_EFBIG, 22);
        assert_eq!(WASI_EINTR, 27);
    }

    #[test]
    fn test_poll_oneoff_rejects_empty_subscription_list() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(poll_oneoff(100, 500, 0, 900, &mut mem, &env), WASI_EINVAL);
        assert_eq!(mem.read_i32(900).unwrap(), 0);
    }

    #[test]
    fn test_poll_oneoff_relative_clock_waits() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        write_clock_subscription(
            &mut mem,
            100,
            0xABCD,
            WASI_CLOCK_MONOTONIC,
            60_000_000,
            false,
        );

        let start = Instant::now();
        assert_eq!(poll_oneoff(100, 500, 1, 900, &mut mem, &env), WASI_ESUCCESS);
        assert!(
            start.elapsed() >= Duration::from_millis(55),
            "poll_oneoff returned early after {:?}",
            start.elapsed()
        );

        assert_eq!(mem.read_i32(900).unwrap(), 1);
        let (userdata, errno, ty, _) = read_event(&mem, 500, 0);
        assert_eq!(userdata, 0xABCD);
        assert_eq!(errno, WASI_ESUCCESS as u16);
        assert_eq!(ty, WASI_EVENTTYPE_CLOCK);
    }

    #[test]
    fn test_poll_oneoff_absolute_deadline_in_the_past_returns_at_once() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        // One second before the epoch-relative now, so it is already due.
        let past = wall_clock_nanos().saturating_sub(1_000_000_000);
        write_clock_subscription(&mut mem, 100, 7, WASI_CLOCK_REALTIME, past, true);

        let start = Instant::now();
        assert_eq!(poll_oneoff(100, 500, 1, 900, &mut mem, &env), WASI_ESUCCESS);
        assert!(start.elapsed() < Duration::from_millis(50));
        assert_eq!(mem.read_i32(900).unwrap(), 1);
        assert_eq!(read_event(&mem, 500, 0).0, 7);
    }

    #[test]
    fn test_poll_oneoff_reports_only_the_clocks_that_came_due() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        // 30ms and 5s: waking for the first must not report the second.
        write_clock_subscription(&mut mem, 100, 1, WASI_CLOCK_MONOTONIC, 30_000_000, false);
        write_clock_subscription(
            &mut mem,
            100 + SUBSCRIPTION_SIZE,
            2,
            WASI_CLOCK_MONOTONIC,
            5_000_000_000,
            false,
        );

        assert_eq!(poll_oneoff(100, 500, 2, 900, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(mem.read_i32(900).unwrap(), 1);
        assert_eq!(read_event(&mem, 500, 0).0, 1);
    }

    #[test]
    fn test_poll_oneoff_invalid_clock_id_fails_only_that_subscription() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        // Clock 3 is thread_cputime_id, which wasmrun does not implement.
        write_clock_subscription(&mut mem, 100, 11, 3, 10_000_000, false);
        write_clock_subscription(
            &mut mem,
            100 + SUBSCRIPTION_SIZE,
            22,
            WASI_CLOCK_MONOTONIC,
            10_000_000,
            false,
        );

        assert_eq!(poll_oneoff(100, 500, 2, 900, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(mem.read_i32(900).unwrap(), 2);
        let bad = read_event(&mem, 500, 0);
        assert_eq!(bad.0, 11);
        assert_eq!(bad.1, WASI_EINVAL as u16);
        let good = read_event(&mem, 500, 1);
        assert_eq!(good.0, 22);
        assert_eq!(good.1, WASI_ESUCCESS as u16);
    }

    #[test]
    fn test_poll_oneoff_stdin_reports_bytes_left() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        env.lock().unwrap().set_stdin(b"hello".to_vec());
        let mut mem = LinearMemory::new(1, None).unwrap();
        write_fd_subscription(&mut mem, 100, 5, WASI_EVENTTYPE_FD_READ, WASI_STDIN_FD);

        assert_eq!(poll_oneoff(100, 500, 1, 900, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(mem.read_i32(900).unwrap(), 1);
        let (userdata, errno, ty, nbytes) = read_event(&mem, 500, 0);
        assert_eq!(userdata, 5);
        assert_eq!(errno, WASI_ESUCCESS as u16);
        assert_eq!(ty, WASI_EVENTTYPE_FD_READ);
        assert_eq!(nbytes, 5);
    }

    #[test]
    fn test_poll_oneoff_ready_fd_beats_a_pending_clock() {
        // A ready descriptor alongside a long timeout must return at once
        // rather than sitting out the clock.
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        env.lock().unwrap().set_stdin(b"x".to_vec());
        let mut mem = LinearMemory::new(1, None).unwrap();
        write_clock_subscription(&mut mem, 100, 1, WASI_CLOCK_MONOTONIC, 5_000_000_000, false);
        write_fd_subscription(
            &mut mem,
            100 + SUBSCRIPTION_SIZE,
            2,
            WASI_EVENTTYPE_FD_READ,
            WASI_STDIN_FD,
        );

        let start = Instant::now();
        assert_eq!(poll_oneoff(100, 500, 2, 900, &mut mem, &env), WASI_ESUCCESS);
        assert!(start.elapsed() < Duration::from_millis(100));
        assert_eq!(mem.read_i32(900).unwrap(), 1);
        assert_eq!(read_event(&mem, 500, 0).0, 2);
    }

    #[test]
    fn test_poll_oneoff_bad_fd() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        write_fd_subscription(&mut mem, 100, 9, WASI_EVENTTYPE_FD_READ, 99);
        assert_eq!(poll_oneoff(100, 500, 1, 900, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(read_event(&mem, 500, 0).1, WASI_EBADF as u16);
    }

    #[test]
    fn test_poll_oneoff_gives_up_when_the_execution_is_cancelled() {
        use std::sync::atomic::AtomicBool;
        // The agent's wall-clock timeout trips this flag. The executor only
        // checks it between instructions, so without this a five second sleep
        // would outlive a one second timeout.
        let flag = Arc::new(AtomicBool::new(false));
        let env = make_env();
        env.lock().unwrap().set_cancel_token(Some(flag.clone()));

        let mut mem = LinearMemory::new(1, None).unwrap();
        write_clock_subscription(&mut mem, 100, 1, WASI_CLOCK_MONOTONIC, 5_000_000_000, false);

        let waker = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            waker.store(true, Ordering::Relaxed);
        });

        let start = Instant::now();
        assert_eq!(poll_oneoff(100, 500, 1, 900, &mut mem, &env), WASI_EINTR);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "cancellation took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn test_sched_yield_succeeds() {
        assert_eq!(sched_yield(), WASI_ESUCCESS);
    }

    #[test]
    fn test_path_readlink_reads_the_target() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("target.txt"), b"contents").unwrap();
        std::os::unix::fs::symlink("target.txt", tmp.path().join("link.txt")).unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"link.txt").unwrap();

        let errno = path_readlink(3, 100, 8, 200, 64, 900, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let used = mem.read_i32(900).unwrap() as usize;
        assert_eq!(
            String::from_utf8(mem.read_bytes(200, used).unwrap()).unwrap(),
            "target.txt"
        );
    }

    #[test]
    fn test_path_readlink_reports_a_buffer_that_is_too_small() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("target.txt"), b"contents").unwrap();
        std::os::unix::fs::symlink("target.txt", tmp.path().join("link.txt")).unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"link.txt").unwrap();

        // "target.txt" is 10 bytes and there is no way to report a partial
        // read, so a 4 byte buffer is an error rather than a truncated answer.
        assert_eq!(
            path_readlink(3, 100, 8, 200, 4, 900, &mut mem, &env),
            WASI_ERANGE
        );
        assert_eq!(mem.read_i32(900).unwrap(), 0);
    }

    #[test]
    fn test_path_readlink_on_a_regular_file_is_not_a_link() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("plain.txt"), b"x").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"plain.txt").unwrap();
        assert_eq!(
            path_readlink(3, 100, 9, 200, 64, 900, &mut mem, &env),
            WASI_EINVAL
        );
    }

    #[test]
    fn test_path_readlink_missing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"nope.txt").unwrap();
        assert_eq!(
            path_readlink(3, 100, 8, 200, 64, 900, &mut mem, &env),
            WASI_ENOENT
        );
    }

    #[test]
    fn test_path_filestat_set_times_sets_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("stamp.txt");
        std::fs::write(&file, b"x").unwrap();

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"stamp.txt").unwrap();

        let when_ns: u64 = 1_000_000_000 * 1_000_000_000;
        let errno = path_filestat_set_times(
            3,
            100,
            9,
            0,
            when_ns,
            WASI_FILESTAT_SET_MTIM,
            &mut mem,
            &env,
        );
        assert_eq!(errno, WASI_ESUCCESS);

        let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
        assert_eq!(
            mtime.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            1_000_000_000
        );
    }

    #[test]
    fn test_path_filestat_set_times_now_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("stamp.txt");
        std::fs::write(&file, b"x").unwrap();
        // Push it into the past so "now" is a visible change.
        let old = std::fs::File::options().write(true).open(&file).unwrap();
        old.set_times(
            std::fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000)),
        )
        .unwrap();
        drop(old);

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"stamp.txt").unwrap();

        let errno =
            path_filestat_set_times(3, 100, 9, 0, 0, WASI_FILESTAT_SET_MTIM_NOW, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let mtime = std::fs::metadata(&file).unwrap().modified().unwrap();
        assert!(mtime.duration_since(UNIX_EPOCH).unwrap().as_secs() > 1_000_000);
    }

    #[test]
    fn test_path_filestat_set_times_rejects_contradictory_flags() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("stamp.txt"), b"x").unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"stamp.txt").unwrap();

        // Asking for both an explicit atime and "now" says two different things.
        assert_eq!(
            path_filestat_set_times(
                3,
                100,
                9,
                5,
                0,
                WASI_FILESTAT_SET_ATIM | WASI_FILESTAT_SET_ATIM_NOW,
                &mut mem,
                &env,
            ),
            WASI_EINVAL
        );
    }

    #[test]
    fn test_path_filestat_set_times_leaves_the_other_stamp_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("stamp.txt");
        std::fs::write(&file, b"x").unwrap();

        let seed = std::fs::File::options().write(true).open(&file).unwrap();
        let atime = UNIX_EPOCH + Duration::from_secs(500_000_000);
        seed.set_times(
            std::fs::FileTimes::new()
                .set_accessed(atime)
                .set_modified(atime),
        )
        .unwrap();
        drop(seed);

        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"stamp.txt").unwrap();

        // Set only mtime; atime must survive.
        let errno = path_filestat_set_times(
            3,
            100,
            9,
            0,
            2_000_000_000 * 1_000_000_000,
            WASI_FILESTAT_SET_MTIM,
            &mut mem,
            &env,
        );
        assert_eq!(errno, WASI_ESUCCESS);

        let meta = std::fs::metadata(&file).unwrap();
        assert_eq!(
            meta.modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            2_000_000_000
        );
        assert_eq!(
            meta.accessed()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            500_000_000
        );
    }

    #[test]
    fn test_path_filestat_set_times_missing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        let mut mem = LinearMemory::new(1, None).unwrap();
        mem.write_bytes(100, b"nope.txt").unwrap();
        assert_eq!(
            path_filestat_set_times(3, 100, 8, 0, 0, WASI_FILESTAT_SET_MTIM, &mut mem, &env),
            WASI_ENOENT
        );
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

    #[test]
    fn test_fd_write_file_size_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        env.lock().unwrap().set_max_file_size(Some(4));
        let mut mem = LinearMemory::new(1, None).unwrap();

        // Open a file for writing
        mem.write_bytes(100, b"big.txt").unwrap();
        let errno = path_open(3, 100, 7, WASI_O_CREAT, 0, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        let fd = mem.read_i32(200).unwrap() as u32;

        // A 5-byte write exceeds the 4-byte cap → EFBIG
        mem.write_bytes(400, b"hello").unwrap();
        mem.write_i32(0, 400).unwrap(); // iovec buf_ptr
        mem.write_i32(4, 5).unwrap(); // iovec buf_len
        let errno = fd_write(fd, 0, 1, 300, &mut mem, &env);
        assert_eq!(errno, WASI_EFBIG);

        // A 4-byte write is allowed
        mem.write_bytes(400, b"okay").unwrap();
        mem.write_i32(4, 4).unwrap();
        let errno = fd_write(fd, 0, 1, 300, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(std::fs::read(tmp.path().join("big.txt")).unwrap(), b"okay");
    }

    /// Open a fresh `O_CREAT` file at `name` and return its fd.
    fn open_create(name: &str, env: &Arc<Mutex<WasiEnv>>, mem: &mut LinearMemory) -> u32 {
        mem.write_bytes(100, name.as_bytes()).unwrap();
        let errno = path_open(3, 100, name.len() as u32, WASI_O_CREAT, 0, 200, mem, env);
        assert_eq!(errno, WASI_ESUCCESS);
        mem.read_i32(200).unwrap() as u32
    }

    /// Write `data` to `fd` at its current offset, returning the syscall errno.
    fn write_bytes_to(
        fd: u32,
        data: &[u8],
        env: &Arc<Mutex<WasiEnv>>,
        mem: &mut LinearMemory,
    ) -> i32 {
        mem.write_bytes(400, data).unwrap();
        mem.write_i32(0, 400).unwrap(); // iovec buf_ptr
        mem.write_i32(4, data.len() as i32).unwrap(); // iovec buf_len
        fd_write(fd, 0, 1, 300, mem, env)
    }

    #[test]
    fn test_fd_write_disk_quota_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        env.lock().unwrap().set_max_disk_bytes(Some(10));
        let mut mem = LinearMemory::new(1, None).unwrap();

        // a.txt = 6 bytes → disk_used = 6.
        let fd_a = open_create("a.txt", &env, &mut mem);
        assert_eq!(
            write_bytes_to(fd_a, b"123456", &env, &mut mem),
            WASI_ESUCCESS
        );
        assert_eq!(env.lock().unwrap().disk_used(), 6);

        // b.txt: a 6-byte write would push total to 12 > 10 → EDQUOT, no change.
        let fd_b = open_create("b.txt", &env, &mut mem);
        assert_eq!(write_bytes_to(fd_b, b"123456", &env, &mut mem), WASI_EDQUOT);
        assert_eq!(env.lock().unwrap().disk_used(), 6);

        // A 4-byte write fits exactly (6 + 4 = 10).
        assert_eq!(write_bytes_to(fd_b, b"okay", &env, &mut mem), WASI_ESUCCESS);
        assert_eq!(env.lock().unwrap().disk_used(), 10);
    }

    #[test]
    fn test_unlink_frees_disk_quota() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        env.lock().unwrap().set_max_disk_bytes(Some(10));
        let mut mem = LinearMemory::new(1, None).unwrap();

        let fd_a = open_create("a.txt", &env, &mut mem);
        assert_eq!(
            write_bytes_to(fd_a, b"12345678", &env, &mut mem),
            WASI_ESUCCESS
        );

        // b.txt 5-byte write → 13 > 10 → EDQUOT.
        let fd_b = open_create("b.txt", &env, &mut mem);
        assert_eq!(write_bytes_to(fd_b, b"hello", &env, &mut mem), WASI_EDQUOT);

        // Unlink a.txt → frees 8, counter back to 0.
        mem.write_bytes(100, b"a.txt").unwrap();
        assert_eq!(path_unlink_file(3, 100, 5, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(env.lock().unwrap().disk_used(), 0);

        // The 5-byte write now fits.
        assert_eq!(
            write_bytes_to(fd_b, b"hello", &env, &mut mem),
            WASI_ESUCCESS
        );
        assert_eq!(env.lock().unwrap().disk_used(), 5);
    }

    #[test]
    fn test_disk_unlimited_never_rejects() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        // No disk cap configured → writes always succeed and the counter stays 0.
        let mut mem = LinearMemory::new(1, None).unwrap();
        let fd = open_create("big.txt", &env, &mut mem);
        for _ in 0..5 {
            assert_eq!(
                write_bytes_to(fd, b"0123456789", &env, &mut mem),
                WASI_ESUCCESS
            );
        }
        assert_eq!(env.lock().unwrap().disk_used(), 0);
    }

    #[test]
    fn test_seed_disk_used_respected() {
        let tmp = tempfile::tempdir().unwrap();
        let env = Arc::new(Mutex::new(WasiEnv::new().with_preopen("/", tmp.path())));
        {
            let mut e = env.lock().unwrap();
            e.set_max_disk_bytes(Some(10));
            e.seed_disk_used(8); // pretend 8 bytes already on disk
        }
        let mut mem = LinearMemory::new(1, None).unwrap();
        let fd = open_create("c.txt", &env, &mut mem);

        // 3-byte write → 8 + 3 = 11 > 10 → EDQUOT.
        assert_eq!(write_bytes_to(fd, b"xyz", &env, &mut mem), WASI_EDQUOT);
        // 2-byte write → 8 + 2 = 10 → ok.
        assert_eq!(write_bytes_to(fd, b"yo", &env, &mut mem), WASI_ESUCCESS);
    }

    // ── Sockets ──────────────────────────────────────────

    /// A `WasiEnv` holding one listener on a free port, plus that port.
    fn env_with_listener() -> (Arc<Mutex<WasiEnv>>, u16) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        (
            Arc::new(Mutex::new(WasiEnv::new().with_tcp_listener(listener))),
            port,
        )
    }

    /// Write one iovec at `iov_at` pointing at `buf_at`, the shape every
    /// read and write syscall takes.
    fn write_iovec(mem: &mut LinearMemory, iov_at: usize, buf_at: usize, len: usize) {
        mem.write_i32(iov_at, buf_at as i32).unwrap();
        mem.write_i32(iov_at + 4, len as i32).unwrap();
    }

    #[test]
    fn test_sock_accept_hands_back_a_connected_fd() {
        let (env, port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();

        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(b"ping").unwrap();
            let mut reply = [0u8; 4];
            stream.read_exact(&mut reply).unwrap();
            reply
        });

        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_ESUCCESS);
        let conn_fd = mem.read_i32(100).unwrap() as u32;
        assert!(conn_fd > 3, "expected a fresh fd, got {conn_fd}");

        // The guest reads with fd_read and writes with fd_write, which is what
        // a stock wasm32-wasip1 binary does with an accepted socket.
        write_iovec(&mut mem, 200, 300, 16);
        assert_eq!(fd_read(conn_fd, 200, 1, 400, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(mem.read_i32(400).unwrap(), 4);
        assert_eq!(&mem.read_bytes(300, 4).unwrap(), b"ping");

        mem.write_bytes(500, b"pong").unwrap();
        write_iovec(&mut mem, 600, 500, 4);
        assert_eq!(
            fd_write(conn_fd, 600, 1, 700, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(mem.read_i32(700).unwrap(), 4);

        assert_eq!(&client.join().unwrap(), b"pong");
    }

    #[test]
    fn test_sock_recv_and_send_are_the_same_socket() {
        let (env, port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();

        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(b"hello").unwrap();
            let mut reply = [0u8; 2];
            stream.read_exact(&mut reply).unwrap();
            reply
        });

        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_ESUCCESS);
        let conn_fd = mem.read_i32(100).unwrap() as u32;

        write_iovec(&mut mem, 200, 300, 32);
        assert_eq!(
            sock_recv(conn_fd, 200, 1, 0, 400, 404, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(mem.read_i32(400).unwrap(), 5);
        assert_eq!(&mem.read_bytes(300, 5).unwrap(), b"hello");
        assert_eq!(mem.read_u16(404).unwrap(), 0, "no roflags on a plain read");

        mem.write_bytes(500, b"ok").unwrap();
        write_iovec(&mut mem, 600, 500, 2);
        assert_eq!(
            sock_send(conn_fd, 600, 1, 0, 700, &mut mem, &env),
            WASI_ESUCCESS
        );
        assert_eq!(&client.join().unwrap(), b"ok");
    }

    #[test]
    fn test_sock_accept_reports_a_non_socket_fd() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();
        // fd 1 is stdout: a descriptor, but not one you can accept on.
        assert_eq!(sock_accept(1, 0, 100, &mut mem, &env), WASI_ENOTSOCK);
        assert_eq!(sock_accept(99, 0, 100, &mut mem, &env), WASI_EBADF);
    }

    #[test]
    fn test_sock_accept_nonblocking_reports_eagain_with_nobody_waiting() {
        let (env, _port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(sock_accept(3, 0x0004, 100, &mut mem, &env), WASI_EAGAIN);
    }

    #[test]
    fn test_a_blocked_accept_gives_up_when_the_execution_is_cancelled() {
        use std::sync::atomic::AtomicBool;
        // The agent's wall-clock timeout trips this flag, and the executor
        // only reads it between instructions: never, while accept waits.
        let (env, _port) = env_with_listener();
        let flag = Arc::new(AtomicBool::new(false));
        env.lock().unwrap().set_cancel_token(Some(flag.clone()));

        let waker = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            waker.store(true, Ordering::Relaxed);
        });

        let mut mem = LinearMemory::new(1, None).unwrap();
        let start = Instant::now();
        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_EINTR);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "cancellation took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn test_poll_reports_a_listener_ready_and_accept_takes_that_connection() {
        let (env, port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();

        let _client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        // Give the connection a moment to land in the accept queue.
        std::thread::sleep(Duration::from_millis(50));

        // A readiness probe has to take the connection to see it, so the one
        // it took must be the one the following accept returns rather than a
        // dropped connection and a hang.
        let (errno, ready) = poll_fd_readiness(3, WASI_EVENTTYPE_FD_READ, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(ready, 1);

        let start = Instant::now();
        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_ESUCCESS);
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "accept should have taken the parked connection immediately"
        );
    }

    #[test]
    fn test_closing_a_socket_fd_drops_the_connection() {
        let (env, port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();

        let client = std::thread::spawn(move || {
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            let mut buf = [0u8; 1];
            // Reads zero at EOF, which is what the guest closing its end means.
            stream.read(&mut buf).unwrap()
        });

        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_ESUCCESS);
        let conn_fd = mem.read_i32(100).unwrap() as u32;
        assert!(env.lock().unwrap().close_fd(conn_fd));
        assert_eq!(client.join().unwrap(), 0);

        // And the fd is gone as far as the guest is concerned.
        write_iovec(&mut mem, 200, 300, 4);
        assert_eq!(fd_read(conn_fd, 200, 1, 400, &mut mem, &env), WASI_EBADF);
    }

    #[test]
    fn test_sock_shutdown_rejects_a_nonsense_direction() {
        let (env, port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();
        let _client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();

        assert_eq!(sock_accept(3, 0, 100, &mut mem, &env), WASI_ESUCCESS);
        let conn_fd = mem.read_i32(100).unwrap() as u32;

        assert_eq!(sock_shutdown(conn_fd, 9, &env), WASI_EINVAL);
        assert_eq!(sock_shutdown(conn_fd, 2, &env), WASI_ESUCCESS);
    }

    #[test]
    fn test_fdstat_calls_a_socket_a_socket() {
        let (env, _port) = env_with_listener();
        let mut mem = LinearMemory::new(1, None).unwrap();
        assert_eq!(fd_fdstat_get(3, 100, &mut mem, &env), WASI_ESUCCESS);
        assert_eq!(mem.read_u8(100).unwrap(), WASI_FILETYPE_SOCKET_STREAM);
    }
}
