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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_is_tool_installed_with_existing_tool() {
        // Test with a tool that should exist on most systems
        assert!(CommandExecutor::is_tool_installed("echo"));
    }

    #[test]
    fn test_is_tool_installed_with_nonexistent_tool() {
        // Test with a tool that shouldn't exist
        assert!(!CommandExecutor::is_tool_installed(
            "nonexistent_tool_12345"
        ));
    }

    #[test]
    fn test_execute_command_success() {
        let temp_dir = tempdir().unwrap();
        let result = CommandExecutor::execute_command(
            "echo",
            &["hello"],
            temp_dir.path().to_str().unwrap(),
            false,
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn test_execute_command_failure() {
        let temp_dir = tempdir().unwrap();
        let result = CommandExecutor::execute_command(
            "nonexistent_command_12345",
            &[],
            temp_dir.path().to_str().unwrap(),
            false,
        );
        assert!(result.is_err());
        match result {
            Err(CompilationError::ToolExecutionFailed { tool, .. }) => {
                assert_eq!(tool, "nonexistent_command_12345");
            }
            _ => panic!("Expected ToolExecutionFailed error"),
        }
    }

    #[test]
    fn test_copy_to_output() {
        let temp_dir = tempdir().unwrap();
        let source_file = temp_dir.path().join("source.wasm");
        let mut file = File::create(&source_file).unwrap();
        file.write_all(b"test wasm content").unwrap();

        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir(&output_dir).unwrap();

        let result = CommandExecutor::copy_to_output(
            source_file.to_str().unwrap(),
            output_dir.to_str().unwrap(),
            "Test",
        );

        assert!(result.is_ok());
        let output_path = result.unwrap();
        assert!(std::path::Path::new(&output_path).exists());
        assert!(output_path.ends_with("source.wasm"));
    }

    #[test]
    fn test_copy_to_output_invalid_source() {
        let temp_dir = tempdir().unwrap();
        let result = CommandExecutor::copy_to_output(
            "/nonexistent/source.wasm",
            temp_dir.path().to_str().unwrap(),
            "Test",
        );

        assert!(result.is_err());
        match result {
            Err(CompilationError::BuildFailed { language, .. }) => {
                assert_eq!(language, "Test");
            }
            _ => panic!("Expected BuildFailed error"),
        }
    }

    #[test]
    fn test_format_file_size_bytes() {
        assert_eq!(CommandExecutor::format_file_size(0), "0 bytes");
        assert_eq!(CommandExecutor::format_file_size(1), "1 bytes");
        assert_eq!(CommandExecutor::format_file_size(1023), "1023 bytes");
    }

    #[test]
    fn test_format_file_size_kilobytes() {
        assert_eq!(CommandExecutor::format_file_size(1024), "1.00 KB");
        assert_eq!(CommandExecutor::format_file_size(1536), "1.50 KB");
        assert_eq!(
            CommandExecutor::format_file_size(1024 * 1024 - 1),
            "1024.00 KB"
        );
    }

    #[test]
    fn test_format_file_size_megabytes() {
        assert_eq!(CommandExecutor::format_file_size(1024 * 1024), "1.00 MB");
        assert_eq!(CommandExecutor::format_file_size(1536 * 1024), "1.50 MB");
        assert_eq!(
            CommandExecutor::format_file_size(1024 * 1024 * 1024 - 1),
            "1024.00 MB"
        );
    }

    #[test]
    fn test_format_file_size_gigabytes() {
        assert_eq!(
            CommandExecutor::format_file_size(1024 * 1024 * 1024),
            "1.00 GB"
        );
        assert_eq!(
            CommandExecutor::format_file_size(1536 * 1024 * 1024),
            "1.50 GB"
        );
        assert_eq!(
            CommandExecutor::format_file_size(2048 * 1024 * 1024),
            "2.00 GB"
        );
    }
}
