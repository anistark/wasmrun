//! WASI (WebAssembly System Interface) integration
//!
//! Registers memory-bridged host functions so the executor can dispatch
//! WASI imports through the linker.

pub mod syscalls;

use crate::runtime::core::executor::WASI_PROC_EXIT_PREFIX;
use crate::runtime::core::linker::{ClosureHostFunction, Linker};
use crate::runtime::core::values::Value;
use std::sync::{Arc, Mutex};

pub struct WasiEnv {
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl WasiEnv {
    pub fn new() -> Self {
        WasiEnv {
            args: Vec::new(),
            env_vars: Vec::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
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
}

impl Default for WasiEnv {
    fn default() -> Self {
        Self::new()
    }
}

const WASI_MODULE: &str = "wasi_snapshot_preview1";

/// Build a [`Linker`] with all supported WASI syscalls registered.
pub fn create_wasi_linker(env: Arc<Mutex<WasiEnv>>) -> Linker {
    let mut linker = Linker::new();

    // ── fd_write ──────────────────────────────────────────────
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

    // ── fd_read ───────────────────────────────────────────────
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

    // ── fd_close ──────────────────────────────────────────────
    linker.register(
        WASI_MODULE,
        "fd_close",
        Box::new(ClosureHostFunction::new(
            |args, _mem| {
                let fd = i32_arg(&args, 0)? as u32;
                Ok(vec![Value::I32(syscalls::fd_close(fd))])
            },
            1,
            1,
        )),
    );

    // ── fd_seek ───────────────────────────────────────────────
    linker.register(
        WASI_MODULE,
        "fd_seek",
        Box::new(ClosureHostFunction::new(
            |args, mem| {
                let fd = i32_arg(&args, 0)? as u32;
                let offset = i64_arg(&args, 1)?;
                let whence = i32_arg(&args, 2)? as u32;
                let newoffset_ptr = i32_arg(&args, 3)? as u32;
                let errno = syscalls::fd_seek(fd, offset, whence, newoffset_ptr, mem);
                Ok(vec![Value::I32(errno)])
            },
            4,
            1,
        )),
    );

    // ── fd_fdstat_get ─────────────────────────────────────────
    linker.register(
        WASI_MODULE,
        "fd_fdstat_get",
        Box::new(ClosureHostFunction::new(
            |args, mem| {
                let fd = i32_arg(&args, 0)? as u32;
                let stat_ptr = i32_arg(&args, 1)? as u32;
                Ok(vec![Value::I32(syscalls::fd_fdstat_get(fd, stat_ptr, mem))])
            },
            2,
            1,
        )),
    );

    // ── fd_prestat_get ────────────────────────────────────────
    linker.register(
        WASI_MODULE,
        "fd_prestat_get",
        Box::new(ClosureHostFunction::new(
            |args, _mem| {
                let fd = i32_arg(&args, 0)? as u32;
                Ok(vec![Value::I32(syscalls::fd_prestat_get(fd))])
            },
            2,
            1,
        )),
    );

    // ── fd_prestat_dir_name ───────────────────────────────────
    linker.register(
        WASI_MODULE,
        "fd_prestat_dir_name",
        Box::new(ClosureHostFunction::new(
            |args, _mem| {
                let fd = i32_arg(&args, 0)? as u32;
                Ok(vec![Value::I32(syscalls::fd_prestat_dir_name(fd))])
            },
            3,
            1,
        )),
    );

    // ── args_sizes_get ────────────────────────────────────────
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

    // ── args_get ──────────────────────────────────────────────
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

    // ── environ_sizes_get ─────────────────────────────────────
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

    // ── environ_get ───────────────────────────────────────────
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

    // ── clock_time_get ────────────────────────────────────────
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

    // ── random_get ────────────────────────────────────────────
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

    // ── proc_exit ─────────────────────────────────────────────
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

    // ── poll_oneoff (stub) ────────────────────────────────────
    linker.register(
        WASI_MODULE,
        "poll_oneoff",
        Box::new(ClosureHostFunction::new(
            |_args, _mem| Ok(vec![Value::I32(syscalls::WASI_ENOSYS)]),
            4,
            1,
        )),
    );

    // ── sched_yield (stub) ────────────────────────────────────
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
        // Many WASI calls pass i32 where i64 is expected
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

        // Set up "Hi" at offset 100, iovec at 0
        mem.write_bytes(100, b"Hi").unwrap();
        mem.write_i32(0, 100).unwrap(); // buf_ptr
        mem.write_i32(4, 2).unwrap(); // buf_len

        let host_fn = linker.get_import(WASI_MODULE, "fd_write").unwrap();
        let result = host_fn
            .call(
                vec![
                    Value::I32(1),  // fd = stdout
                    Value::I32(0),  // iovs
                    Value::I32(1),  // iovs_len
                    Value::I32(16), // nwritten
                ],
                &mut mem,
            )
            .unwrap();

        assert_eq!(result[0], Value::I32(0)); // ESUCCESS
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
    fn test_wasi_env_builder() {
        let env = WasiEnv::new()
            .with_args(vec!["prog".into(), "a".into()])
            .with_env("K".into(), "V".into());

        assert_eq!(env.args(), &["prog", "a"]);
        assert_eq!(env.env_vars(), &[("K".into(), "V".into())]);
        assert!(env.get_stdout().is_empty());
        assert!(env.get_stderr().is_empty());
    }
}
