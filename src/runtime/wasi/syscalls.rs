//! WASI syscall implementations that operate on linear memory.
#![allow(dead_code)]

use crate::runtime::core::memory::LinearMemory;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::WasiEnv;

// WASI errno constants
pub const WASI_ESUCCESS: i32 = 0;
pub const WASI_E2BIG: i32 = 1;
pub const WASI_EACCES: i32 = 2;
pub const WASI_EBADF: i32 = 8;
pub const WASI_EINVAL: i32 = 28;
pub const WASI_EIO: i32 = 29;
pub const WASI_ENOSYS: i32 = 52;
pub const WASI_ENOENT: i32 = 44;

pub const WASI_STDIN_FD: u32 = 0;
pub const WASI_STDOUT_FD: u32 = 1;
pub const WASI_STDERR_FD: u32 = 2;

pub const WASI_CLOCK_REALTIME: u32 = 0;
pub const WASI_CLOCK_MONOTONIC: u32 = 1;

// WASI file types
pub const WASI_FILETYPE_CHARACTER_DEVICE: u8 = 2;
pub const WASI_FILETYPE_DIRECTORY: u8 = 3;
pub const WASI_FILETYPE_REGULAR_FILE: u8 = 4;

// fdstat struct layout (24 bytes):
//   offset 0: filetype  (u8)
//   offset 2: fdflags   (u16)
//   offset 8: rights_base       (u64)
//   offset 16: rights_inheriting (u64)

/// fd_write — read iovec structs from memory, capture output.
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
            _ => return WASI_EBADF,
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

/// fd_read — for stdin returns 0 bytes (non-interactive).
pub fn fd_read(
    fd: u32,
    _iovs_ptr: u32,
    _iovs_len: u32,
    nread_ptr: u32,
    memory: &mut LinearMemory,
    _env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    match fd {
        WASI_STDIN_FD => {
            if memory.write_i32(nread_ptr as usize, 0).is_err() {
                return WASI_EINVAL;
            }
            WASI_ESUCCESS
        }
        _ => WASI_EBADF,
    }
}

/// fd_close — close a file descriptor.
pub fn fd_close(fd: u32) -> i32 {
    match fd {
        WASI_STDIN_FD | WASI_STDOUT_FD | WASI_STDERR_FD => WASI_ESUCCESS,
        _ => WASI_EBADF,
    }
}

/// fd_seek — not supported for character devices.
pub fn fd_seek(
    fd: u32,
    _offset: i64,
    _whence: u32,
    newoffset_ptr: u32,
    memory: &mut LinearMemory,
) -> i32 {
    match fd {
        WASI_STDIN_FD | WASI_STDOUT_FD | WASI_STDERR_FD => {
            if memory.write_i64(newoffset_ptr as usize, 0).is_err() {
                return WASI_EINVAL;
            }
            WASI_ESUCCESS
        }
        _ => WASI_EBADF,
    }
}

/// fd_fdstat_get — write file descriptor status to memory.
pub fn fd_fdstat_get(fd: u32, stat_ptr: u32, memory: &mut LinearMemory) -> i32 {
    let (filetype, flags, rights) = match fd {
        WASI_STDIN_FD => (WASI_FILETYPE_CHARACTER_DEVICE, 0u16, 0x200u64), // FD_READ right
        WASI_STDOUT_FD | WASI_STDERR_FD => {
            (WASI_FILETYPE_CHARACTER_DEVICE, 1u16, 0x400u64) // FD_WRITE right, APPEND flag
        }
        _ => return WASI_EBADF,
    };

    let base = stat_ptr as usize;
    // Zero the struct first (24 bytes)
    for i in 0..24 {
        if memory.write_u8(base + i, 0).is_err() {
            return WASI_EINVAL;
        }
    }
    if memory.write_u8(base, filetype).is_err() {
        return WASI_EINVAL;
    }
    if memory.write_u16(base + 2, flags).is_err() {
        return WASI_EINVAL;
    }
    if memory.write_i64(base + 8, rights as i64).is_err() {
        return WASI_EINVAL;
    }
    if memory.write_i64(base + 16, rights as i64).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// fd_prestat_get — report no preopened directories for now.
pub fn fd_prestat_get(_fd: u32) -> i32 {
    WASI_EBADF
}

/// fd_prestat_dir_name — no preopened directories.
pub fn fd_prestat_dir_name(_fd: u32) -> i32 {
    WASI_EBADF
}

/// args_sizes_get — write argument count and total buffer size to memory.
pub fn args_sizes_get(
    count_ptr: u32,
    buf_size_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let (argc, buf_size) = if let Ok(e) = env.lock() {
        let args = e.args();
        let total: usize = args.iter().map(|a| a.len() + 1).sum(); // +1 for NUL
        (args.len() as i32, total as i32)
    } else {
        (0, 0)
    };

    if memory.write_i32(count_ptr as usize, argc).is_err() {
        return WASI_EINVAL;
    }
    if memory.write_i32(buf_size_ptr as usize, buf_size).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// args_get — write argv pointers and string data to memory.
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
        // Write pointer to this arg string
        let ptr_addr = argv_ptr as usize + i * 4;
        if memory.write_i32(ptr_addr, buf_offset as i32).is_err() {
            return WASI_EINVAL;
        }
        // Write the arg string + NUL
        if memory.write_bytes(buf_offset, arg.as_bytes()).is_err() {
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

/// environ_sizes_get — write env var count and total buffer size.
pub fn environ_sizes_get(
    count_ptr: u32,
    buf_size_ptr: u32,
    memory: &mut LinearMemory,
    env: &Arc<Mutex<WasiEnv>>,
) -> i32 {
    let (count, buf_size) = if let Ok(e) = env.lock() {
        let vars = e.env_vars();
        let total: usize = vars.iter().map(|(k, v)| k.len() + 1 + v.len() + 1).sum(); // KEY=VALUE\0
        (vars.len() as i32, total as i32)
    } else {
        (0, 0)
    };

    if memory.write_i32(count_ptr as usize, count).is_err() {
        return WASI_EINVAL;
    }
    if memory.write_i32(buf_size_ptr as usize, buf_size).is_err() {
        return WASI_EINVAL;
    }
    WASI_ESUCCESS
}

/// environ_get — write env var pointers and "KEY=VALUE\0" strings.
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
        if memory.write_i32(ptr_addr, buf_offset as i32).is_err() {
            return WASI_EINVAL;
        }
        let entry = format!("{key}={value}");
        if memory.write_bytes(buf_offset, entry.as_bytes()).is_err() {
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

/// clock_time_get — write nanosecond timestamp to memory.
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

/// random_get — fill buffer with pseudo-random bytes.
pub fn random_get(buf_ptr: u32, buf_len: u32, memory: &mut LinearMemory) -> i32 {
    let seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => 0x12345678,
    };
    let mut state = seed;
    for i in 0..buf_len {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let byte = (state & 0xFF) as u8;
        if memory
            .write_u8(buf_ptr as usize + i as usize, byte)
            .is_err()
        {
            return WASI_EINVAL;
        }
    }
    WASI_ESUCCESS
}

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

        // Data at offset 100: "Hello"
        mem.write_bytes(100, b"Hello").unwrap();
        // iovec at offset 0: { buf_ptr=100, buf_len=5 }
        mem.write_i32(0, 100).unwrap();
        mem.write_i32(4, 5).unwrap();

        let errno = fd_write(WASI_STDOUT_FD, 0, 1, 16, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);

        let nwritten = mem.read_i32(16).unwrap();
        assert_eq!(nwritten, 5);

        let captured = env.lock().unwrap().get_stdout();
        assert_eq!(captured, b"Hello");
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

        let captured = env.lock().unwrap().get_stderr();
        assert_eq!(captured, b"error msg");
    }

    #[test]
    fn test_fd_write_multiple_iovecs() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();

        mem.write_bytes(200, b"Hello, ").unwrap();
        mem.write_bytes(300, b"World!").unwrap();
        // iovec[0] at 0:  { 200, 7 }
        mem.write_i32(0, 200).unwrap();
        mem.write_i32(4, 7).unwrap();
        // iovec[1] at 8:  { 300, 6 }
        mem.write_i32(8, 300).unwrap();
        mem.write_i32(12, 6).unwrap();

        let errno = fd_write(WASI_STDOUT_FD, 0, 2, 32, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);

        let nwritten = mem.read_i32(32).unwrap();
        assert_eq!(nwritten, 13);

        let captured = env.lock().unwrap().get_stdout();
        assert_eq!(captured, b"Hello, World!");
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
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = fd_fdstat_get(WASI_STDOUT_FD, 0, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_u8(0).unwrap(), WASI_FILETYPE_CHARACTER_DEVICE);
    }

    #[test]
    fn test_fd_prestat_get_returns_ebadf() {
        assert_eq!(fd_prestat_get(3), WASI_EBADF);
        assert_eq!(fd_prestat_get(4), WASI_EBADF);
    }

    #[test]
    fn test_args_sizes_get_empty() {
        let env = make_env();
        let mut mem = LinearMemory::new(1, None).unwrap();

        let errno = args_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 0); // argc
        assert_eq!(mem.read_i32(4).unwrap(), 0); // buf_size
    }

    #[test]
    fn test_args_roundtrip() {
        let env = make_env_with_args(vec!["prog".into(), "hello".into()]);
        let mut mem = LinearMemory::new(1, None).unwrap();

        let errno = args_sizes_get(0, 4, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);
        assert_eq!(mem.read_i32(0).unwrap(), 2); // 2 args
        assert_eq!(mem.read_i32(4).unwrap(), 11); // "prog\0" + "hello\0" = 5+6

        // argv pointers at 100, string buffer at 200
        let errno = args_get(100, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);

        let ptr0 = mem.read_i32(100).unwrap() as usize;
        let ptr1 = mem.read_i32(104).unwrap() as usize;

        let s0 = mem.read_bytes(ptr0, 4).unwrap();
        assert_eq!(&s0, b"prog");
        assert_eq!(mem.read_u8(ptr0 + 4).unwrap(), 0); // NUL

        let s1 = mem.read_bytes(ptr1, 5).unwrap();
        assert_eq!(&s1, b"hello");
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
        assert_eq!(mem.read_i32(0).unwrap(), 2); // 2 env vars

        let errno = environ_get(100, 200, &mut mem, &env);
        assert_eq!(errno, WASI_ESUCCESS);

        let ptr0 = mem.read_i32(100).unwrap() as usize;
        let s0 = mem.read_bytes(ptr0, 7).unwrap();
        assert_eq!(&s0, b"FOO=bar");
    }

    #[test]
    fn test_clock_time_get_realtime() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = clock_time_get(WASI_CLOCK_REALTIME, 0, 0, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        let nanos = mem.read_i64(0).unwrap();
        assert!(nanos > 0);
    }

    #[test]
    fn test_random_get_fills_buffer() {
        let mut mem = LinearMemory::new(1, None).unwrap();
        let errno = random_get(0, 16, &mut mem);
        assert_eq!(errno, WASI_ESUCCESS);
        let bytes = mem.read_bytes(0, 16).unwrap();
        // Very unlikely all 16 bytes are zero
        assert!(bytes.iter().any(|&b| b != 0));
    }
}
