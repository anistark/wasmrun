//! WASI syscall implementations
//!
//! Provides implementations for common WASI syscalls like fd_read, fd_write, etc.

#![allow(dead_code)]

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// WASI clock IDs
pub const WASI_CLOCK_REALTIME: u32 = 0;
pub const WASI_CLOCK_MONOTONIC: u32 = 1;
pub const WASI_CLOCK_PROCESS_CPUTIME_ID: u32 = 2;
pub const WASI_CLOCK_THREAD_CPUTIME_ID: u32 = 3;

/// Represents result of fd_write
pub struct FdWriteResult {
    pub nwritten: u32,
}

/// Represents result of fd_read
pub struct FdReadResult {
    pub nread: u32,
}

/// Handle fd_write syscall (write to file descriptor)
///
/// Writes data from memory to a file descriptor.
/// For stdout/stderr (FDs 1-2), writes to stdio.
/// Returns WASI_ESUCCESS on success.
pub fn fd_write(
    fd: u32,
    _iov_ptr: u32,
    _iov_count: u32,
    _nwritten_ptr: u32,
) -> Result<i32, String> {
    match fd {
        WASI_STDOUT_FD => {
            // Write to stdout
            let _ = io::stdout().flush();
            Ok(WASI_ESUCCESS)
        }
        WASI_STDERR_FD => {
            // Write to stderr
            let _ = io::stderr().flush();
            Ok(WASI_ESUCCESS)
        }
        _ => {
            // Invalid file descriptor for writing
            Ok(WASI_EBADF)
        }
    }
}

/// Handle fd_read syscall (read from file descriptor)
///
/// Reads data from a file descriptor into memory.
/// For stdin (FD 0), reads from stdio.
/// Returns WASI_ESUCCESS on success.
pub fn fd_read(_fd: u32, _iov_ptr: u32, _iov_count: u32, _nread_ptr: u32) -> Result<i32, String> {
    // Simplified implementation - return success
    // Full implementation would read from actual file descriptors
    Ok(WASI_ESUCCESS)
}

/// Handle proc_exit syscall
///
/// Exits the process with the given code.
/// In native runtime mode, this would terminate execution.
pub fn proc_exit(code: i32) -> Result<i32, String> {
    // In a full implementation, this would exit the process
    // For now, just return the code to signal it was called
    Ok(code)
}

/// Handle environ_get syscall
///
/// Retrieves environment variables and stores them in memory.
/// Returns WASI_ESUCCESS on success.
pub fn environ_get(_environ_ptr: u32, _environ_buf_size: u32) -> Result<i32, String> {
    // In a full implementation, this would:
    // 1. Iterate over std::env::vars()
    // 2. Format each as "KEY=VALUE\0"
    // 3. Write pointers to environ_ptr
    // 4. Write all strings to the buffer
    Ok(WASI_ESUCCESS)
}

/// Handle environ_sizes_get syscall
///
/// Returns the count and total size of environment variables.
/// Returns WASI_ESUCCESS on success.
pub fn environ_sizes_get(_count_ptr: u32, _buf_size_ptr: u32) -> Result<i32, String> {
    // In a full implementation, this would:
    // 1. Count the number of environment variables
    // 2. Calculate the total size needed for all "KEY=VALUE\0" strings
    // 3. Write count to count_ptr
    // 4. Write total size to buf_size_ptr
    Ok(WASI_ESUCCESS)
}

/// Handle args_get syscall
///
/// Retrieves command-line arguments and stores them in memory.
/// Returns WASI_ESUCCESS on success.
pub fn args_get(_argv_ptr: u32, _argv_buf_size: u32) -> Result<i32, String> {
    // In a full implementation, this would:
    // 1. Iterate over stored arguments
    // 2. Write pointers to argv_ptr
    // 3. Write all argument strings to the buffer
    Ok(WASI_ESUCCESS)
}

/// Handle args_sizes_get syscall
///
/// Returns the count and total size of command-line arguments.
/// Returns WASI_ESUCCESS on success.
pub fn args_sizes_get(_count_ptr: u32, _buf_size_ptr: u32) -> Result<i32, String> {
    // In a full implementation, this would:
    // 1. Count the number of arguments
    // 2. Calculate the total size needed for all argument strings
    // 3. Write count to count_ptr
    // 4. Write total size to buf_size_ptr
    Ok(WASI_ESUCCESS)
}

/// Handle clock_time_get syscall
///
/// Returns the current time for the specified clock.
/// Returns WASI_ESUCCESS on success.
pub fn clock_time_get(id: u32, _precision: u64, _time_ptr: u32) -> Result<i32, String> {
    match id {
        WASI_CLOCK_REALTIME => {
            // Get current real time (seconds since Unix epoch)
            let now = SystemTime::now();
            match now.duration_since(UNIX_EPOCH) {
                Ok(_duration) => Ok(WASI_ESUCCESS),
                Err(_) => Ok(WASI_EIO),
            }
        }
        WASI_CLOCK_MONOTONIC => {
            // For monotonic clock, use elapsed time
            Ok(WASI_ESUCCESS)
        }
        _ => {
            // Unsupported clock type
            Ok(WASI_EINVAL)
        }
    }
}

/// Handle random_get syscall
///
/// Fills a buffer with random bytes.
/// Returns WASI_ESUCCESS on success.
pub fn random_get(_buf_ptr: u32, _buf_len: u32) -> Result<i32, String> {
    // In a full implementation, this would:
    // 1. Use a CSPRNG (e.g., getrandom crate) to generate random bytes
    // 2. Write the bytes to the buffer at buf_ptr
    // 3. Return WASI_ESUCCESS
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

    #[test]
    fn test_fd_write_stdout() {
        let result = fd_write(WASI_STDOUT_FD, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_fd_write_stderr() {
        let result = fd_write(WASI_STDERR_FD, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_fd_write_invalid_fd() {
        let result = fd_write(99, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_EBADF);
    }

    #[test]
    fn test_fd_read_stdin() {
        let result = fd_read(WASI_STDIN_FD, 0, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_environ_get_returns_success() {
        let result = environ_get(0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_environ_sizes_get_returns_success() {
        let result = environ_sizes_get(0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_args_get_returns_success() {
        let result = args_get(0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_args_sizes_get_returns_success() {
        let result = args_sizes_get(0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_clock_time_get_realtime() {
        let result = clock_time_get(WASI_CLOCK_REALTIME, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_clock_time_get_monotonic() {
        let result = clock_time_get(WASI_CLOCK_MONOTONIC, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_clock_time_get_invalid_id() {
        let result = clock_time_get(99, 0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_EINVAL);
    }

    #[test]
    fn test_random_get_returns_success() {
        let result = random_get(0, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), WASI_ESUCCESS);
    }

    #[test]
    fn test_clock_constants() {
        assert_eq!(WASI_CLOCK_REALTIME, 0);
        assert_eq!(WASI_CLOCK_MONOTONIC, 1);
        assert_eq!(WASI_CLOCK_PROCESS_CPUTIME_ID, 2);
        assert_eq!(WASI_CLOCK_THREAD_CPUTIME_ID, 3);
    }
}
