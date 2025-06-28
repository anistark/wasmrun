use thiserror::Error;

/// The main error type for Wasmrun operations
#[derive(Error, Debug)]
pub enum WasmrunError {
    /// I/O related errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Path-related errors
    #[error("Path error: {message}")]
    Path { message: String },

    /// File not found
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    /// Directory not found
    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: String },

    /// Invalid file format
    #[error("Invalid file format: {path} - {reason}")]
    InvalidFileFormat { path: String, reason: String },

    /// WASM-specific errors
    #[error(transparent)]
    Wasm(#[from] WasmError),

    /// Compilation errors
    #[error(transparent)]
    Compilation(#[from] CompilationError),

    /// Server errors
    #[error(transparent)]
    Server(#[from] ServerError),

    /// Command execution errors
    #[error(transparent)]
    Command(#[from] CommandError),

    /// Configuration errors
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Language detection errors
    #[error("Language detection failed: {message}")]
    LanguageDetection { message: String },

    /// Multiple tools missing
    #[error("Missing required tools: {tools:?}")]
    MissingTools { tools: Vec<String> },

    /// Generic error with context
    #[error("{context}: {source}")]
    WithContext {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// WASM-specific errors
#[derive(Error, Debug)]
pub enum WasmError {
    /// Invalid WASM magic bytes
    #[error("Invalid WASM magic bytes in file: {path}")]
    InvalidMagicBytes { path: String },

    /// WASM module validation failed
    #[error("WASM module validation failed: {reason}")]
    ValidationFailed { reason: String },

    /// wasm-bindgen detection
    #[error("wasm-bindgen module detected but JavaScript file not found")]
    WasmBindgenJsNotFound,
}

/// Compilation-related errors
#[derive(Error, Debug)]
pub enum CompilationError {
    /// Language not supported
    #[error("Language not supported: {language}")]
    UnsupportedLanguage { language: String },

    /// Build tool not found
    #[error("Build tool not found: {tool}. Please install {tool} to compile {language} projects")]
    BuildToolNotFound { tool: String, language: String },

    /// Build failed
    #[error("Build failed for {language} project: {reason}")]
    BuildFailed { language: String, reason: String },

    /// Tool execution failed
    #[error("Failed to execute {tool}: {reason}")]
    ToolExecutionFailed { tool: String, reason: String },

    /// Project structure invalid
    #[error("Invalid {language} project structure: {reason}")]
    InvalidProjectStructure { language: String, reason: String },

    /// Missing entry file
    #[error("No entry file found for {language} project. Expected one of: {candidates:?}")]
    MissingEntryFile {
        language: String,
        candidates: Vec<String>,
    },

    /// Output directory creation failed
    #[error("Failed to create output directory: {path}")]
    OutputDirectoryCreationFailed { path: String },

    /// Optimization level invalid
    #[error("Invalid optimization level: {level}. Valid options: {valid_options:?}")]
    InvalidOptimizationLevel {
        level: String,
        valid_options: Vec<String>,
    },
}

/// Server-related errors
#[derive(Error, Debug)]
pub enum ServerError {
    /// Server startup failed
    #[error("Failed to start server on port {port}: {reason}")]
    StartupFailed { port: u16, reason: String },

    /// Request handling failed
    #[error("Failed to handle request: {reason}")]
    RequestHandlingFailed { reason: String },

    /// Server not running
    #[error("No server is currently running")]
    NotRunning,

    /// Failed to stop server
    #[error("Failed to stop server with PID {pid}: {reason}")]
    StopFailed { pid: u32, reason: String },
}

/// Command execution errors
#[derive(Error, Debug)]
pub enum CommandError {
    /// Invalid arguments
    #[error("Invalid command arguments: {message}")]
    InvalidArguments { message: String },
}

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration error")]
    _Placeholder,
}

/// Result type alias for Wasmrun operations
pub type Result<T> = std::result::Result<T, WasmrunError>;

/// Specialized result types for different modules
pub type CompilationResult<T> = std::result::Result<T, CompilationError>;

impl WasmrunError {
    /// new path error
    pub fn path(message: impl Into<String>) -> Self {
        Self::Path {
            message: message.into(),
        }
    }

    /// file not found error
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    /// directory not found error
    pub fn directory_not_found(path: impl Into<String>) -> Self {
        Self::DirectoryNotFound { path: path.into() }
    }

    /// Invalid file format error
    pub fn invalid_file_format(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidFileFormat {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// language detection error
    pub fn language_detection(message: impl Into<String>) -> Self {
        Self::LanguageDetection {
            message: message.into(),
        }
    }

    /// missing tools error
    pub fn missing_tools(tools: Vec<String>) -> Self {
        Self::MissingTools { tools }
    }

    /// Add context to an error
    pub fn add_context<E>(context: impl Into<String>, error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::WithContext {
            context: context.into(),
            source: Box::new(error),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            WasmrunError::FileNotFound { .. } => false,
            WasmrunError::DirectoryNotFound { .. } => false,
            WasmrunError::MissingTools { .. } => false,
            _ => false,
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            WasmrunError::FileNotFound { path } => {
                format!("File not found: {}\n💡 Check the file path and try again", path)
            }
            WasmrunError::DirectoryNotFound { path } => {
                format!("Directory not found: {}\n💡 Check the directory path and try again", path)
            }
            WasmrunError::MissingTools { tools } => {
                format!(
                    "Missing required tools: {}\n💡 Please install these tools to continue",
                    tools.join(", ")
                )
            }
            WasmrunError::Wasm(WasmError::WasmBindgenJsNotFound) => {
                "This appears to be a wasm-bindgen module\n💡 Try running the corresponding .js file instead".to_string()
            }
            _ => self.to_string(),
        }
    }

    /// Get suggested actions for the error
    pub fn suggestions(&self) -> Vec<String> {
        match self {
            WasmrunError::MissingTools { tools } => tools
                .iter()
                .map(|tool| format!("Install {} using your package manager", tool))
                .collect(),
            WasmrunError::Compilation(CompilationError::MissingEntryFile { candidates, .. }) => {
                vec![
                    format!("Create one of these entry files: {}", candidates.join(", ")),
                    "Check your project structure".to_string(),
                    "Refer to the language documentation".to_string(),
                ]
            }
            _ => vec![],
        }
    }
}

impl WasmError {
    /// new validation failed error
    pub fn validation_failed(reason: impl Into<String>) -> Self {
        Self::ValidationFailed {
            reason: reason.into(),
        }
    }
}

impl CompilationError {
    /// new build failed error
    pub fn build_failed(language: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::BuildFailed {
            language: language.into(),
            reason: reason.into(),
        }
    }
}

impl ServerError {
    /// new startup failed error
    pub fn startup_failed(port: u16, reason: impl Into<String>) -> Self {
        Self::StartupFailed {
            port,
            reason: reason.into(),
        }
    }
}

impl CommandError {
    /// new invalid arguments error
    pub fn invalid_arguments(message: impl Into<String>) -> Self {
        Self::InvalidArguments {
            message: message.into(),
        }
    }
}

impl From<&str> for WasmrunError {
    fn from(message: &str) -> Self {
        WasmrunError::Path {
            message: message.to_string(),
        }
    }
}
