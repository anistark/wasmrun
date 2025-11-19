//! WASI (WebAssembly System Interface) integration for native runtime
//!
//! This module provides WASI syscall implementations that can be imported into WASM modules

pub mod syscalls;

use crate::runtime::core::linker::{ClosureHostFunction, Linker};
use crate::runtime::core::values::Value;
use std::sync::{Arc, Mutex};

/// WASI environment containing syscall state
#[allow(dead_code)]
pub struct WasiEnv {
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    stdout: Arc<Mutex<Vec<u8>>>,
    stderr: Arc<Mutex<Vec<u8>>>,
}

impl WasiEnv {
    /// Create a new WASI environment
    pub fn new() -> Self {
        WasiEnv {
            args: Vec::new(),
            env_vars: Vec::new(),
            stdout: Arc::new(Mutex::new(Vec::new())),
            stderr: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set command-line arguments
    #[allow(dead_code)]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add an environment variable
    #[allow(dead_code)]
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_vars.push((key, value));
        self
    }

    /// Get stdout content
    #[allow(dead_code)]
    pub fn get_stdout(&self) -> Vec<u8> {
        self.stdout.lock().unwrap().clone()
    }

    /// Get stderr content
    #[allow(dead_code)]
    pub fn get_stderr(&self) -> Vec<u8> {
        self.stderr.lock().unwrap().clone()
    }
}

impl Default for WasiEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a WASI linker with all syscalls registered
#[allow(dead_code)]
pub fn create_wasi_linker(_env: Arc<Mutex<WasiEnv>>) -> Linker {
    let mut linker = Linker::new();

    // Register proc_exit syscall (1 arg: exit code, 0 returns)
    linker.register(
        "proc_exit".to_string(),
        Box::new(ClosureHostFunction::new(
            |args| {
                if args.is_empty() {
                    return Ok(vec![]);
                }
                match &args[0] {
                    Value::I32(_code) => {
                        // In a real implementation, this would exit the process
                        // For now, we just return success
                        Ok(vec![])
                    }
                    _ => Err("proc_exit expects i32 exit code".to_string()),
                }
            },
            1,
            0,
        )),
    );

    // Register fd_write syscall (4 args: fd, iov_ptr, iov_count, nwritten_ptr; 1 return: errno)
    linker.register(
        "fd_write".to_string(),
        Box::new(ClosureHostFunction::new(
            |args| {
                if args.len() < 4 {
                    return Ok(vec![Value::I32(syscalls::WASI_EINVAL)]);
                }
                let fd = match &args[0] {
                    Value::I32(v) => *v as u32,
                    _ => return Ok(vec![Value::I32(syscalls::WASI_EBADF)]),
                };
                Ok(vec![Value::I32(
                    syscalls::fd_write(fd, 0, 0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            4,
            1,
        )),
    );

    // Register fd_read syscall (4 args: fd, iov_ptr, iov_count, nread_ptr; 1 return: errno)
    linker.register(
        "fd_read".to_string(),
        Box::new(ClosureHostFunction::new(
            |args| {
                if args.len() < 4 {
                    return Ok(vec![Value::I32(syscalls::WASI_EINVAL)]);
                }
                let fd = match &args[0] {
                    Value::I32(v) => *v as u32,
                    _ => return Ok(vec![Value::I32(syscalls::WASI_EBADF)]),
                };
                Ok(vec![Value::I32(
                    syscalls::fd_read(fd, 0, 0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            4,
            1,
        )),
    );

    // Register environ_sizes_get syscall (2 args: count_ptr, buf_size_ptr; 1 return: errno)
    linker.register(
        "environ_sizes_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |_args| {
                Ok(vec![Value::I32(
                    syscalls::environ_sizes_get(0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            2,
            1,
        )),
    );

    // Register environ_get syscall (2 args: environ_ptr, buf_size; 1 return: errno)
    linker.register(
        "environ_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |_args| {
                Ok(vec![Value::I32(
                    syscalls::environ_get(0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            2,
            1,
        )),
    );

    // Register args_sizes_get syscall (2 args: count_ptr, buf_size_ptr; 1 return: errno)
    linker.register(
        "args_sizes_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |_args| {
                Ok(vec![Value::I32(
                    syscalls::args_sizes_get(0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            2,
            1,
        )),
    );

    // Register args_get syscall (2 args: argv_ptr, buf_size; 1 return: errno)
    linker.register(
        "args_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |_args| {
                Ok(vec![Value::I32(
                    syscalls::args_get(0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            2,
            1,
        )),
    );

    // Register clock_time_get syscall (3 args: clock_id, precision, time_ptr; 1 return: errno)
    linker.register(
        "clock_time_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |args| {
                if args.len() < 3 {
                    return Ok(vec![Value::I32(syscalls::WASI_EINVAL)]);
                }
                let clock_id = match &args[0] {
                    Value::I32(v) => *v as u32,
                    _ => return Ok(vec![Value::I32(syscalls::WASI_EINVAL)]),
                };
                Ok(vec![Value::I32(
                    syscalls::clock_time_get(clock_id, 0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            3,
            1,
        )),
    );

    // Register random_get syscall (2 args: buf_ptr, buf_len; 1 return: errno)
    linker.register(
        "random_get".to_string(),
        Box::new(ClosureHostFunction::new(
            |_args| {
                Ok(vec![Value::I32(
                    syscalls::random_get(0, 0).unwrap_or(syscalls::WASI_EIO),
                )])
            },
            2,
            1,
        )),
    );

    linker
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasi_env_creation() {
        let env = WasiEnv::new();
        assert_eq!(env.args.len(), 0);
        assert_eq!(env.env_vars.len(), 0);
    }

    #[test]
    fn test_wasi_env_with_args() {
        let env = WasiEnv::new()
            .with_args(vec!["prog".to_string(), "arg1".to_string()])
            .with_env("VAR".to_string(), "value".to_string());

        assert_eq!(env.args.len(), 2);
        assert_eq!(env.env_vars.len(), 1);
    }

    #[test]
    fn test_wasi_linker_creation() {
        let env = Arc::new(Mutex::new(WasiEnv::new()));
        let linker = create_wasi_linker(env);

        assert!(linker.has("proc_exit"));
        assert!(linker.has("fd_write"));
        assert!(linker.has("fd_read"));
        assert!(linker.has("environ_get"));
        assert!(linker.has("environ_sizes_get"));
        assert!(linker.has("args_get"));
        assert!(linker.has("args_sizes_get"));
        assert!(linker.has("clock_time_get"));
        assert!(linker.has("random_get"));
    }

    #[test]
    fn test_wasi_env_get_stdout() {
        let env = WasiEnv::new();
        let stdout = env.get_stdout();
        assert!(stdout.is_empty());
    }

    #[test]
    fn test_wasi_env_get_stderr() {
        let env = WasiEnv::new();
        let stderr = env.get_stderr();
        assert!(stderr.is_empty());
    }

    #[test]
    fn test_wasi_env_with_multiple_env_vars() {
        let env = WasiEnv::new()
            .with_env("VAR1".to_string(), "value1".to_string())
            .with_env("VAR2".to_string(), "value2".to_string())
            .with_env("VAR3".to_string(), "value3".to_string());

        assert_eq!(env.env_vars.len(), 3);
    }
}
