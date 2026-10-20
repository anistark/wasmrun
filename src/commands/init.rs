//! [Shared] `wasmrun init`: a new project from a template.
//!
//! Each template is one of the repository's examples, compiled into the binary,
//! so a scaffold is a project CI already builds and runs. The example's own
//! name is replaced with the new project's wherever it appears.

use crate::cli::CommandValidator;
use crate::error::{Result, WasmrunError};
use crate::ui::print_init_info;
use std::path::Path;

/// One template: the example it comes from, the files it writes, and how to
/// run the result.
struct Template {
    name: &'static str,
    /// The example's name as its files spell it, replaced by the project's.
    example: &'static str,
    /// Other names the example goes by (a package.json `name`), replaced too.
    aliases: &'static [&'static str],
    files: &'static [(&'static str, &'static str)],
    mode: &'static str,
    next: &'static [&'static str],
}

/// An example's file, through the copy `build.rs` makes in `templates/init/`,
/// which is what the crate package carries. `build.rs` reads its list of files
/// from these calls.
macro_rules! example_file {
    ($example:literal, $path:literal) => {
        (
            $path,
            include_str!(concat!(
                "../../templates/init/",
                $example,
                "/",
                $path,
                ".tmpl"
            )),
        )
    };
}

const TEMPLATES: &[Template] = &[
    Template {
        name: "rust",
        aliases: &[],
        example: "rust-hello",
        files: &[
            example_file!("rust-hello", "Cargo.toml"),
            example_file!("rust-hello", "src/lib.rs"),
            example_file!("rust-hello", "README.md"),
            example_file!("rust-hello", ".gitignore"),
        ],
        mode: "server mode",
        next: &["wasmrun {dir}"],
    },
    Template {
        name: "go",
        aliases: &[],
        example: "go-hello",
        files: &[
            example_file!("go-hello", "go.mod"),
            example_file!("go-hello", "main.go"),
            example_file!("go-hello", "README.md"),
        ],
        mode: "server mode",
        next: &["wasmrun {dir}"],
    },
    Template {
        name: "c",
        aliases: &[],
        example: "c-hello",
        files: &[
            example_file!("c-hello", "Makefile"),
            example_file!("c-hello", "main.c"),
            example_file!("c-hello", "README.md"),
        ],
        mode: "server mode",
        next: &["wasmrun {dir}"],
    },
    Template {
        name: "asc",
        aliases: &[],
        example: "asc-hello",
        files: &[
            example_file!("asc-hello", "package.json"),
            example_file!("asc-hello", "asconfig.json"),
            example_file!("asc-hello", "assembly/index.ts"),
            example_file!("asc-hello", "README.md"),
        ],
        mode: "server mode",
        next: &["wasmrun {dir}"],
    },
    Template {
        name: "python",
        aliases: &[],
        example: "python-hello",
        files: &[
            example_file!("python-hello", "main.py"),
            example_file!("python-hello", "README.md"),
        ],
        mode: "server mode",
        next: &["wasmrun {dir}"],
    },
    Template {
        name: "nodejs",
        aliases: &["wasmrun-http-api-example"],
        example: "nodejs-http-api",
        files: &[
            example_file!("nodejs-http-api", "package.json"),
            example_file!("nodejs-http-api", "index.js"),
            example_file!("nodejs-http-api", "public/index.html"),
            example_file!("nodejs-http-api", "README.md"),
            example_file!("nodejs-http-api", ".gitignore"),
        ],
        mode: "OS mode, or agent mode's serve",
        next: &["wasmrun os {dir}"],
    },
    Template {
        name: "wasi",
        aliases: &[],
        example: "native-rust",
        files: &[
            example_file!("native-rust", "Cargo.toml"),
            example_file!("native-rust", "src/main.rs"),
        ],
        mode: "exec mode",
        next: &[
            "cd {dir} && cargo build --release --target wasm32-wasip1",
            "wasmrun exec target/wasm32-wasip1/release/{crate}.wasm",
        ],
    },
];

/// Every template name, for `--template` and its error message.
pub fn template_names() -> Vec<&'static str> {
    TEMPLATES.iter().map(|t| t.name).collect()
}

/// Handle init command
pub fn handle_init_command(
    name: &Option<String>,
    template: &str,
    directory: &Option<String>,
) -> Result<()> {
    let (project_name, template_name, target_dir) =
        CommandValidator::validate_init_args(name, template, directory)?;
    let template = TEMPLATES
        .iter()
        .find(|t| t.name == template_name)
        .expect("validated above");

    print_init_info(&project_name, &template_name, &target_dir);
    let written = scaffold(template, &project_name, Path::new(&target_dir))?;
    for file in &written {
        println!("  + {file}");
    }

    let crate_name = project_name.replace('-', "_");
    println!(
        "\n✅ Created {project_name} from the {template_name} template ({}). Next:",
        template.mode
    );
    for step in template.next {
        println!(
            "   {}",
            step.replace("{dir}", &target_dir)
                .replace("{crate}", &crate_name)
        );
    }
    Ok(())
}

/// Write a template's files under `dir`, which must not exist yet, and return
/// their relative paths.
fn scaffold(template: &Template, project_name: &str, dir: &Path) -> Result<Vec<String>> {
    let underscored_example = template.example.replace('-', "_");
    let underscored_name = project_name.replace('-', "_");
    let mut written = Vec::new();
    for (path, content) in template.files {
        let mut content = content
            .replace(
                &format!("examples/{}", template.example),
                &dir.display().to_string(),
            )
            .replace(template.example, project_name)
            .replace(&underscored_example, &underscored_name);
        for alias in template.aliases {
            content = content.replace(alias, project_name);
        }
        let dest = dir.join(path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                WasmrunError::from(format!("Failed to create {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&dest, content)
            .map_err(|e| WasmrunError::from(format!("Failed to write {}: {e}", dest.display())))?;
        written.push(path.to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_scaffolds_with_the_project_name() {
        for template in TEMPLATES {
            let root = tempfile::tempdir().unwrap();
            let dir = root.path().join("my-app");
            let written = scaffold(template, "my-app", &dir).unwrap();
            assert_eq!(written.len(), template.files.len(), "{}", template.name);
            for file in written {
                let content = std::fs::read_to_string(dir.join(&file)).unwrap();
                for old in std::iter::once(&template.example).chain(template.aliases) {
                    assert!(
                        !content.contains(old),
                        "{} still names {old} in {file}",
                        template.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_rust_template_takes_the_crate_name() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("hello-wasm");
        let rust = TEMPLATES.iter().find(|t| t.name == "rust").unwrap();
        scaffold(rust, "hello-wasm", &dir).unwrap();
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"hello-wasm\""), "{cargo}");
    }

    #[test]
    fn template_names_cover_every_mode() {
        let names = template_names();
        for expected in ["rust", "go", "c", "asc", "python", "nodejs", "wasi"] {
            assert!(names.contains(&expected), "{expected}");
        }
    }
}
