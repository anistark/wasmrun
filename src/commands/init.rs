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

    println!("📦 Creating new {template_name} project: {project_name}");
    println!("📂 Target directory: {target_dir}");

    // Create the project directory
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| WasmrunError::from(format!("Failed to create directory {target_dir}: {e}")))?;

    // Create project files based on template
    // match template_name.as_str() {
    //     "rust" => create_rust_project(&target_dir, &project_name)?,
    //     "go" => create_go_project(&target_dir, &project_name)?,
    //     "c" => create_c_project(&target_dir, &project_name)?,
    //     "asc" => create_asc_project(&target_dir, &project_name)?,
    //     "python" => create_python_project(&target_dir, &project_name)?,
    //     _ => {
    //         return Err(WasmrunError::from(format!(
    //             "Unknown template: {template_name}"
    //         )));
    //     }
    // }

    // println!("✅ Project '{project_name}' created successfully!");
    // println!("🚀 To get started:");
    // println!("   cd {target_dir}");
    // println!("   wasmrun");

    println!("🛠️  Project file generation feature coming soon!");

    Ok(())
}

// fn create_rust_project(target_dir: &str, project_name: &str) -> Result<()> {
//     // Create Cargo.toml
//     let cargo_toml = format!(
//         r#"[package]
// name = "{project_name}"
// version = "0.1.0"
// edition = "2021"

// [lib]
// crate-type = ["cdylib"]

// [dependencies]
// wasm-bindgen = "0.2"

// [dependencies.web-sys]
// version = "0.3"
// features = [
//   "console",
// ]
// "#
//     );
//     std::fs::write(format!("{target_dir}/Cargo.toml"), cargo_toml)
//         .map_err(|e| WasmrunError::from(format!("Failed to write Cargo.toml: {e}")))?;

//     // Create src directory and lib.rs
//     std::fs::create_dir_all(format!("{target_dir}/src"))
//         .map_err(|e| WasmrunError::from(format!("Failed to create src directory: {e}")))?;

//     let lib_rs = r#"use wasm_bindgen::prelude::*;

// // Import the `console.log` function from the browser
// #[wasm_bindgen]
// extern "C" {
//     #[wasm_bindgen(js_namespace = console)]
//     fn log(s: &str);
// }

// // Define a macro to make it easier to call console.log
// macro_rules! console_log {
//     ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
// }

// // Export a `greet` function from Rust to JavaScript
// #[wasm_bindgen]
// pub fn greet(name: &str) {
//     console_log!("Hello, {}!", name);
// }

// // Export an `add` function
// #[wasm_bindgen]
// pub fn add(a: i32, b: i32) -> i32 {
//     a + b
// }
// "#;
//     std::fs::write(format!("{target_dir}/src/lib.rs"), lib_rs)
//         .map_err(|e| WasmrunError::from(format!("Failed to write lib.rs: {e}")))?;

//     Ok(())
// }

// fn create_go_project(target_dir: &str, project_name: &str) -> Result<()> {
//     // Create go.mod
//     let go_mod = format!("module {project_name}\n\ngo 1.19\n");
//     std::fs::write(format!("{target_dir}/go.mod"), go_mod)
//         .map_err(|e| WasmrunError::from(format!("Failed to write go.mod: {e}")))?;

//     // Create main.go
//     let main_go = r#"package main

// import "fmt"

// //export add
// func add(x, y int) int {
//     return x + y
// }

// //export greet
// func greet(name string) {
//     fmt.Printf("Hello, %s!\n", name)
// }

// func main() {
//     // Main function required but not used in WASM
// }
// "#;
//     std::fs::write(format!("{target_dir}/main.go"), main_go)
//         .map_err(|e| WasmrunError::from(format!("Failed to write main.go: {e}")))?;

//     Ok(())
// }

// fn create_c_project(target_dir: &str, project_name: &str) -> Result<()> {
//     // Create Makefile
//     let makefile = format!(
//         r#"CC = clang
// TARGET = {project_name}.wasm
// SOURCE = main.c

// $(TARGET): $(SOURCE)
// 	$(CC) --target=wasm32 -O3 -flto -nostdlib -Wl,--no-entry -Wl,--export-all -o $(TARGET) $(SOURCE)

// clean:
// 	rm -f $(TARGET)

// .PHONY: clean
// "#
//     );
//     std::fs::write(format!("{target_dir}/Makefile"), makefile)
//         .map_err(|e| WasmrunError::from(format!("Failed to write Makefile: {e}")))?;

//     // Create main.c
//     let main_c = r#"// Simple C WASM example

// int add(int a, int b) {
//     return a + b;
// }

// int multiply(int a, int b) {
//     return a * b;
// }

// int factorial(int n) {
//     if (n <= 1) return 1;
//     return n * factorial(n - 1);
// }
// "#;
//     std::fs::write(format!("{target_dir}/main.c"), main_c)
//         .map_err(|e| WasmrunError::from(format!("Failed to write main.c: {e}")))?;

//     Ok(())
// }

// fn create_asc_project(target_dir: &str, project_name: &str) -> Result<()> {
//     // Create package.json
//     let package_json = format!(
//         r#"{{
//   "name": "{project_name}",
//   "version": "1.0.0",
//   "description": "AssemblyScript WASM project",
//   "scripts": {{
//     "build": "asc assembly/index.ts --target release"
//   }},
//   "devDependencies": {{
//     "assemblyscript": "^0.20.0"
//   }}
// }}
// "#
//     );
//     std::fs::write(format!("{target_dir}/package.json"), package_json)
//         .map_err(|e| WasmrunError::from(format!("Failed to write package.json: {e}")))?;

//     // Create assembly directory and index.ts
//     std::fs::create_dir_all(format!("{target_dir}/assembly"))
//         .map_err(|e| WasmrunError::from(format!("Failed to create assembly directory: {e}")))?;

//     let index_ts = r#"// AssemblyScript WASM example

// export function add(a: i32, b: i32): i32 {
//   return a + b;
// }

// export function multiply(a: i32, b: i32): i32 {
//   return a * b;
// }

// export function fibonacci(n: i32): i32 {
//   if (n <= 1) return n;
//   return fibonacci(n - 1) + fibonacci(n - 2);
// }
// "#;
//     std::fs::write(format!("{target_dir}/assembly/index.ts"), index_ts)
//         .map_err(|e| WasmrunError::from(format!("Failed to write index.ts: {e}")))?;

//     Ok(())
// }

// fn create_python_project(target_dir: &str, project_name: &str) -> Result<()> {
//     // Create main.py
//     let main_py = format!(
//         r#"# {project_name} - Python to WASM project

// def add(a, b):
//     """Add two numbers."""
//     return a + b

// def multiply(a, b):
//     """Multiply two numbers."""
//     return a * b

// def greet(name):
//     """Greet a person."""
//     print(f"Hello, {{name}}!")

// if __name__ == "__main__":
//     print("Python WASM project created!")
//     print(f"2 + 3 = {{add(2, 3)}}")
//     greet("World")
// "#
//     );
//     std::fs::write(format!("{target_dir}/main.py"), main_py)
//         .map_err(|e| WasmrunError::from(format!("Failed to write main.py: {e}")))?;

//     // Create requirements.txt
//     let requirements = "# Python WASM requirements\n# Add your dependencies here\n";
//     std::fs::write(format!("{target_dir}/requirements.txt"), requirements)
//         .map_err(|e| WasmrunError::from(format!("Failed to write requirements.txt: {e}")))?;

//     Ok(())
// }
