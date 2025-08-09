use crate::cli::CommandValidator;
use crate::error::{Result, WasmrunError};
use crate::ui::print_init_info;

/// Handle init command
#[allow(dead_code)]
pub fn handle_init_command(
    name: &Option<String>,
    template: &str,
    directory: &Option<String>,
) -> Result<()> {
    let (project_name, template_name, target_dir) =
        CommandValidator::validate_init_args(name, template, directory)?;

    print_init_info(&project_name, &template_name, &target_dir);

    // TODO: Implement project initialization
    // Need to create a new project from a template. (Can be plugin based?)
    println!("📦 Creating new {template_name} project: {project_name}");
    println!("📂 Target directory: {target_dir}");

    Err(WasmrunError::from(
        "Project initialization is not yet implemented. This will be added in a future version."
            .to_string(),
    ))
}
