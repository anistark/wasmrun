//! WASI syscall implementations
//!
//! Provides implementations for common WASI syscalls like fd_read, fd_write, etc.

#![allow(dead_code)]

/// WASI errno values (subset of common ones)
pub const WASI_ESUCCESS: i32 = 0;
pub const WASI_EBADF: i32 = 8;
pub const WASI_EINVAL: i32 = 28;
pub const WASI_EIO: i32 = 5;
pub const WASI_ENOSYS: i32 = 52;

/// File descriptor numbers reserved by WASI
pub const WASI_STDIN_FD: u32 = 0;
pub const WASI_STDOUT_FD: u32 = 1;
pub const WASI_STDERR_FD: u32 = 2;

/// Represents result of fd_write
pub struct FdWriteResult {
    pub nwritten: u32,
}

/// Represents result of fd_read
pub struct FdReadResult {
    pub nread: u32,
}

/// Handle fd_write syscall (write to file descriptor)
pub fn fd_write(
    _fd: u32,
    _iov_ptr: u32,
    _iov_count: u32,
    _nwritten_ptr: u32,
) -> Result<i32, String> {
    // Simplified implementation - return success
    Ok(WASI_ESUCCESS)
}

/// Handle fd_read syscall (read from file descriptor)
pub fn fd_read(_fd: u32, _iov_ptr: u32, _iov_count: u32, _nread_ptr: u32) -> Result<i32, String> {
    // Simplified implementation - return success
    Ok(WASI_ESUCCESS)
}

/// Handle proc_exit syscall
pub fn proc_exit(code: i32) -> Result<i32, String> {
    // In a full implementation, this would exit the process
    // For now, just return the code
    Ok(code)
}

/// Handle environ_get syscall
pub fn environ_get(_environ_ptr: u32, _environ_buf_size: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

/// Handle environ_sizes_get syscall
pub fn environ_sizes_get(_count_ptr: u32, _buf_size_ptr: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

/// Handle args_get syscall
pub fn args_get(_argv_ptr: u32, _argv_buf_size: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

/// Handle args_sizes_get syscall
pub fn args_sizes_get(_count_ptr: u32, _buf_size_ptr: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

/// Handle clock_time_get syscall
pub fn clock_time_get(_id: u32, _precision: u64, _time_ptr: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

/// Handle random_get syscall
pub fn random_get(_buf_ptr: u32, _buf_len: u32) -> Result<i32, String> {
    Ok(WASI_ESUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_errno_values() {
        assert_eq!(WASI_ESUCCESS, 0);
        assert_eq!(WASI_EBADF, 8);
        assert_eq!(WASI_EINVAL, 28);
    }

    #[test]
    fn test_fd_constants() {
        assert_eq!(WASI_STDIN_FD, 0);
        assert_eq!(WASI_STDOUT_FD, 1);
        assert_eq!(WASI_STDERR_FD, 2);
    }

    #[test]
    fn test_proc_exit() {
        let result = proc_exit(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_fd_read_returns_success() {
        let result = fd_read(0, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_fd_write_returns_success() {
        let result = fd_write(1, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }
}
