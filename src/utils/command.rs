use crate::error::{CompilationError, CompilationResult};

/// Shared builder command utilities
pub struct CommandExecutor;

impl CommandExecutor {
    /// Check if a tool is installed on the system
    pub fn is_tool_installed(tool_name: &str) -> bool {
        let command = if cfg!(target_os = "windows") {
            format!("where {tool_name}")
        } else {
            format!("which {tool_name}")
        };

        std::process::Command::new(if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        })
        .args(if cfg!(target_os = "windows") {
            ["/c", &command]
        } else {
            ["-c", &command]
        })
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    }

    /// Execute a command and return the result
    pub fn execute_command(
        command: &str,
        args: &[&str],
        working_dir: &str,
        verbose: bool,
    ) -> CompilationResult<std::process::Output> {
        if verbose {
            println!("🔧 Executing: {} {}", command, args.join(" "));
        }

        std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .output()
            .map_err(|e| CompilationError::ToolExecutionFailed {
                tool: command.to_string(),
                reason: e.to_string(),
            })
    }

    /// Execute a command with live output
    #[allow(dead_code)]
    pub fn execute_command_with_output(
        command: &str,
        args: &[&str],
        working_dir: &str,
    ) -> CompilationResult<()> {
        println!("🔧 Executing: {} {}", command, args.join(" "));

        let status = std::process::Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .map_err(|e| CompilationError::ToolExecutionFailed {
                tool: command.to_string(),
                reason: e.to_string(),
            })?;

        if !status.success() {
            return Err(CompilationError::BuildFailed {
                language: "Unknown".to_string(),
                reason: format!(
                    "Command '{}' failed with exit code: {:?}",
                    command,
                    status.code()
                ),
            });
        }

        Ok(())
    }

    /// Copy output file to the target directory
    pub fn copy_to_output(
        source: &str,
        output_dir: &str,
        language: &str,
    ) -> CompilationResult<String> {
        use crate::utils::PathResolver;
        use std::fs;
        use std::path::Path;

        let source_path = Path::new(source);
        let filename =
            PathResolver::get_filename(source).map_err(|_| CompilationError::BuildFailed {
                language: language.to_string(),
                reason: format!("Invalid source file path: {source}"),
            })?;
        let output_path = PathResolver::join_paths(output_dir, &filename);

        fs::copy(source_path, &output_path).map_err(|e| CompilationError::BuildFailed {
            language: language.to_string(),
            reason: format!("Failed to copy {source} to {output_path}: {e}"),
        })?;

        Ok(output_path)
    }

    /// Format file size in human readable format
    pub fn format_file_size(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{bytes} bytes")
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}
