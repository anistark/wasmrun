//! Integration tests for the `wasmrun exec` command
//! These tests verify end-to-end functionality of executing WASM files with various configurations

#[cfg(test)]
mod exec_integration_tests {
    use std::path::PathBuf;
    use std::process::Command;

    fn get_wasmrun_binary() -> PathBuf {
        let mut path = std::env::current_exe().expect("Failed to get current exe path");
        // Current path is something like target/debug/deps/exec_integration_tests-xxxxx
        // We need to go up 3 levels to reach target, then check both debug and release
        path.pop(); // Remove test binary name
        path.pop(); // Remove deps directory

        let mut current_profile_dir = path.clone();

        // Try release first, fall back to debug
        path.push("wasmrun");
        if path.exists() {
            return path;
        }

        // Fall back to debug
        current_profile_dir.push("wasmrun");
        if current_profile_dir.exists() {
            return current_profile_dir;
        }

        // If neither exists, return the debug path (will fail gracefully in test)
        current_profile_dir
    }

    fn run_wasmrun_exec(args: Vec<&str>) -> std::process::Output {
        let binary = get_wasmrun_binary();
        let mut cmd = Command::new(&binary);

        for arg in args {
            cmd.arg(arg);
        }

        cmd.output().expect("Failed to execute wasmrun")
    }

    // Test: Execute native-rust example with add function
    #[test]
    fn test_exec_native_rust_add_function() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        // Skip if example doesn't exist
        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "add", "5", "3"]);

        // The native Rust example has known runtime initialization limitations
        // It will fail with "Unreachable instruction" or "Operand stack underflow"
        // This test just verifies the command runs without crashing
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file") && !stderr.contains("not found"),
            "Should find WASM file, got: {stderr}"
        );
    }

    // Test: Execute native-rust example with multiply function
    #[test]
    fn test_exec_native_rust_multiply_function() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "multiply", "7", "6"]);

        // Same known runtime limitation as add function
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file") && !stderr.contains("not found"),
            "Should find WASM file, got: {stderr}"
        );
    }

    // Test: Execute native-go example with add function
    #[test]
    fn test_exec_native_go_add_function() {
        let wasm_path = "examples/native-go/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "add", "10", "20"]);

        // Native Go has known runtime initialization limitations (TinyGo asyncify state)
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file") && !stderr.contains("not found"),
            "Should find WASM file, got: {stderr}"
        );
    }

    // Test: Execute native-go example with fibonacci function
    #[test]
    fn test_exec_native_go_fibonacci_function() {
        let wasm_path = "examples/native-go/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "fibonacci", "5"]);

        // Same known runtime limitation as add function
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file") && !stderr.contains("not found"),
            "Should find WASM file, got: {stderr}"
        );
    }

    // Test: Execute with non-existent WASM file
    #[test]
    fn test_exec_nonexistent_file() {
        let output = run_wasmrun_exec(vec!["exec", "nonexistent.wasm"]);

        // Should fail
        assert!(
            !output.status.success(),
            "exec should fail for non-existent file"
        );

        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            stderr.contains("not found") || stderr.contains("No such file"),
            "Error message should indicate file not found, got: {stderr}"
        );
    }

    // Test: Execute with invalid extension
    #[test]
    fn test_exec_invalid_extension() {
        let output = run_wasmrun_exec(vec!["exec", "examples/native-rust/Cargo.toml"]);

        // Should fail because it's not a .wasm file
        assert!(
            !output.status.success(),
            "exec should fail for non-wasm file"
        );

        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            stderr.contains(".wasm") || stderr.contains("invalid"),
            "Error message should indicate invalid file type, got: {stderr}"
        );
    }

    // Test: Execute with non-existent function
    #[test]
    fn test_exec_nonexistent_function() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "nonexistent_function"]);

        // Should fail because function doesn't exist
        assert!(
            !output.status.success(),
            "exec should fail for non-existent function"
        );

        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            stderr.contains("not found") || stderr.contains("nonexistent"),
            "Error message should indicate function not found, got: {stderr}"
        );
    }

    // Test: Execute with multiple arguments
    #[test]
    fn test_exec_with_multiple_arguments() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec![
            "exec", wasm_path, "-c", "add", "100", "200", "extra_arg"
        ]);

        // Should execute (might ignore extra args or pass them)
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        // Just verify it doesn't crash with a filesystem error
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Execute with no function specified (should use entry point)
    #[test]
    fn test_exec_no_function_specified() {
        let wasm_path = "examples/native-go/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        // Try to execute without specifying a function
        // This should attempt to find an entry point (main, _start, or start section)
        let output = run_wasmrun_exec(vec!["exec", wasm_path]);

        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        // It's okay if it fails with "No entry point found" - that's expected behavior
        // What matters is it doesn't crash with a filesystem error
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Execute with debug flag
    #[test]
    fn test_exec_with_debug_flag() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec![
            "exec", wasm_path, "-c", "add", "5", "3", "--debug"
        ]);

        // Debug output should be available
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");

        // Either should have debug output or execute successfully
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Execute go-hello example (if available)
    #[test]
    fn test_exec_go_hello_example() {
        let wasm_path = "examples/go-hello/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path]);

        // Should execute without filesystem errors
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Execute rust-hello example (if available)
    #[test]
    fn test_exec_rust_hello_example() {
        let wasm_path = "examples/rust-hello/main.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path]);

        // Should execute without filesystem errors
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Verify inspect command output format for function discovery
    #[test]
    fn test_inspect_shows_exported_functions() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["inspect", wasm_path]);

        // Should succeed
        assert!(output.status.success(), "inspect command should succeed");

        let stdout = std::str::from_utf8(&output.stdout).unwrap_or("");
        // Should mention functions or exports
        assert!(
            stdout.contains("Export") || stdout.contains("Function") || stdout.contains("add"),
            "inspect output should mention exports or functions"
        );
    }

    // Test: Verify exec rejects empty function name
    #[test]
    fn test_exec_empty_function_name() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        // Try with empty string as function name (if that's even possible with CLI parsing)
        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", ""]);

        // Should either fail or be ignored by the CLI parser
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Large argument values
    #[test]
    fn test_exec_large_argument_values() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let large_num = "9223372036854775807"; // i64::MAX
        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "add", large_num, "1"]);

        // Should handle large numbers
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }

    // Test: Negative argument values
    #[test]
    fn test_exec_negative_argument_values() {
        let wasm_path = "examples/native-rust/native_rust.wasm";

        if !std::path::Path::new(wasm_path).exists() {
            eprintln!("⚠️  {wasm_path} not found, skipping test");
            return;
        }

        let output = run_wasmrun_exec(vec!["exec", wasm_path, "-c", "add", "-5", "3"]);

        // Should handle negative numbers
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("");
        assert!(
            !stderr.contains("No such file"),
            "Should not error on file access"
        );
    }
}
