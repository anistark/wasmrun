use crate::cli::CommandValidator;
use crate::server;

/// Handle run command
pub fn handle_run_command(
    path: &Option<String>,
    positional_path: &Option<String>,
    port: u16,
    language: &Option<String>,
    watch: bool,
) -> Result<(), String> {
    let (project_path, validated_port) =
        CommandValidator::validate_run_args(path, positional_path, port)?;

    server::run_project(&project_path, validated_port, language.clone(), watch);
    Ok(())
}
