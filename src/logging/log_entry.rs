use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogSource {
    Kernel,
    WasmExecution,
    DevServer,
    Filesystem,
    Syscall,
    LanguageRuntime(String),
    Unknown,
}

impl fmt::Display for LogSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogSource::Kernel => write!(f, "KERNEL"),
            LogSource::WasmExecution => write!(f, "WASM"),
            LogSource::DevServer => write!(f, "DEV_SERVER"),
            LogSource::Filesystem => write!(f, "FS"),
            LogSource::Syscall => write!(f, "SYSCALL"),
            LogSource::LanguageRuntime(lang) => write!(f, "{}", lang.to_uppercase()),
            LogSource::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    pub pid: Option<u32>,
}

impl LogEntry {
    #[allow(dead_code)]
    pub fn new(level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        let now = chrono::Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        Self {
            timestamp,
            level,
            source,
            message: message.into(),
            pid: None,
        }
    }

    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    #[allow(dead_code)]
    pub fn debug(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Debug, source, message)
    }

    pub fn info(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, source, message)
    }

    #[allow(dead_code)]
    pub fn warn(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warn, source, message)
    }

    pub fn error(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, source, message)
    }
}
