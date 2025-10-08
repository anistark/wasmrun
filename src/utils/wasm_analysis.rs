use crate::commands::{verify_wasm, VerificationResult};
use crate::error::{Result, WasmrunError};
use crate::utils::{CommandExecutor, PathResolver};
use std::fs;
use std::path::Path;

/// Comprehensive WASM file analysis for CLI display
#[derive(Debug)]
pub struct WasmAnalysis {
    pub path: String,
    pub filename: String,
    pub file_size: String,
    #[allow(dead_code)]
    pub file_size_bytes: u64,
    #[allow(dead_code)]
    pub verification: Option<VerificationResult>,
    pub is_valid: bool,
    pub entry_points: Vec<String>,
    #[allow(dead_code)]
    pub is_wasm_bindgen: bool,
    #[allow(dead_code)]
    pub is_wasi: bool,
    pub module_type: ModuleType,
    #[allow(dead_code)]
    pub imports_count: usize,
    pub exports_count: usize,
    pub functions_count: usize,
}

#[derive(Debug, Clone)]
pub enum ModuleType {
    StandardWasm,
    WasmBindgen,
    WasiModule,
    #[allow(dead_code)]
    WebApplication,
    Unknown,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleType::StandardWasm => write!(f, "Standard WebAssembly"),
            ModuleType::WasmBindgen => write!(f, "WASM-Bindgen Module"),
            ModuleType::WasiModule => write!(f, "WASI Module"),
            ModuleType::WebApplication => write!(f, "Web Application"),
            ModuleType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl WasmAnalysis {
    pub fn analyze(path: &str) -> Result<Self> {
        let path_obj = Path::new(path);

        // Validate file exists and has correct extension
        PathResolver::validate_wasm_file(path)?;

        let filename = PathResolver::get_filename(path)?;
        let file_size_bytes = fs::metadata(path)
            .map_err(|e| WasmrunError::add_context(format!("Getting file size for {path}"), e))?
            .len();

        let file_size = CommandExecutor::format_file_size(file_size_bytes);

        // Verify wasm
        let verification = verify_wasm(path).ok();

        let is_valid = verification.as_ref().is_some_and(|v| v.valid_magic);

        // Analyze entry points
        let entry_points = if let Some(ref verify_result) = verification {
            extract_entry_points(verify_result)
        } else {
            Vec::new()
        };

        // Determine module characteristics
        let is_wasm_bindgen = detect_wasm_bindgen(path_obj);
        let is_wasi = verification.as_ref().is_some_and(|v| {
            v.has_export_section && v.export_names.iter().any(|name| name == "_start")
        });

        // Determine module type
        let module_type = determine_module_type(&verification, is_wasm_bindgen, is_wasi);

        let (imports_count, exports_count, functions_count) =
            if let Some(ref verify_result) = verification {
                (
                    0,
                    verify_result.export_names.len(),
                    verify_result.function_count,
                )
            } else {
                (0, 0, 0)
            };

        Ok(WasmAnalysis {
            path: path.to_string(),
            filename,
            file_size,
            file_size_bytes,
            verification,
            is_valid,
            entry_points,
            is_wasm_bindgen,
            is_wasi,
            module_type,
            imports_count,
            exports_count,
            functions_count,
        })
    }

    /// Print comprehensive analysis to console
    pub fn print_analysis(&self) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  🔍 \x1b[1;36mWASM File Analysis\x1b[0m                                     \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        println!("\x1b[1;34m│\x1b[0m  📦 \x1b[1;34mFile:\x1b[0m \x1b[1;33m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 self.filename);
        println!("\x1b[1;34m│\x1b[0m  📂 \x1b[1;34mPath:\x1b[0m \x1b[0;37m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 self.path);
        println!("\x1b[1;34m│\x1b[0m  💾 \x1b[1;34mSize:\x1b[0m \x1b[1;33m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 self.file_size);

        if self.is_valid {
            println!("\x1b[1;34m│\x1b[0m  ✅ \x1b[1;34mStatus:\x1b[0m \x1b[1;32mValid WebAssembly{:<32}\x1b[0m \x1b[1;34m│\x1b[0m", "");
            println!("\x1b[1;34m│\x1b[0m  🏷️  \x1b[1;34mType:\x1b[0m \x1b[1;36m{:<49}\x1b[0m \x1b[1;34m│\x1b[0m", 
                     self.module_type.to_string());
            println!("\x1b[1;34m│\x1b[0m  📊 \x1b[1;34mExports:\x1b[0m \x1b[1;33m{:<47}\x1b[0m \x1b[1;34m│\x1b[0m", 
                     self.exports_count);
            println!("\x1b[1;34m│\x1b[0m  🔧 \x1b[1;34mFunctions:\x1b[0m \x1b[1;33m{:<45}\x1b[0m \x1b[1;34m│\x1b[0m", 
                     self.functions_count);
        } else {
            println!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;34mStatus:\x1b[0m \x1b[1;31mInvalid Format{:<36}\x1b[0m \x1b[1;34m│\x1b[0m", "");
        }

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    /// Get summary of WASM Module
    pub fn get_summary(&self) -> String {
        if !self.is_valid {
            return format!("❌ Invalid WASM file ({})", self.file_size);
        }

        let type_indicator = match self.module_type {
            ModuleType::WasiModule => "🔧",
            ModuleType::WasmBindgen => "🌐",
            ModuleType::WebApplication => "📱",
            _ => "⚡",
        };

        let entry_info = if !self.entry_points.is_empty() {
            format!(" • Entry: {}", self.entry_points[0])
        } else {
            String::new()
        };

        format!(
            "{} {} ({} • {} exports{})",
            type_indicator, self.module_type, self.file_size, self.exports_count, entry_info
        )
    }
}

/// Comprehensive project analysis for directories
#[derive(Debug)]
pub struct ProjectAnalysis {
    pub path: String,
    pub project_name: String,
    pub language: crate::compiler::ProjectLanguage,
    // pub is_web_app: bool,
    #[allow(dead_code)]
    pub has_cargo_toml: bool,
    pub entry_files: Vec<String>,
    pub build_files: Vec<String>,
}

impl ProjectAnalysis {
    /// Analyze a project directory
    pub fn analyze(path: &str) -> Result<Self> {
        PathResolver::validate_directory_exists(path)?;

        let project_name = Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let language = crate::compiler::detect_project_language(path);
        // let is_web_app = language == crate::compiler::ProjectLanguage::Rust
        //     && crate::compiler::is_rust_web_application(path);

        let mut entry_files = Vec::new();
        let mut build_files = Vec::new();
        let mut has_cargo_toml = false;

        // Common files
        let important_files = [
            ("Cargo.toml", true),
            ("package.json", true),
            ("Makefile", true),
            ("go.mod", true),
            ("main.rs", false),
            ("lib.rs", false),
            ("main.go", false),
            ("main.c", false),
            ("index.ts", false),
            ("index.js", false),
        ];

        for (filename, is_build_file) in &important_files {
            let file_path = PathResolver::join_paths(path, filename);
            if Path::new(&file_path).exists() {
                if *filename == "Cargo.toml" {
                    has_cargo_toml = true;
                }

                if *is_build_file {
                    build_files.push(filename.to_string());
                } else {
                    entry_files.push(filename.to_string());
                }
            }
        }

        Ok(ProjectAnalysis {
            path: path.to_string(),
            project_name,
            language,
            // is_web_app,
            has_cargo_toml,
            entry_files,
            build_files,
        })
    }

    /// Print comprehensive project analysis
    pub fn print_analysis(&self) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  📁 \x1b[1;36mProject Analysis\x1b[0m                                       \x1b[1;34m│\x1b[0m");
        println!(
            "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
        );

        println!("\x1b[1;34m│\x1b[0m  📦 \x1b[1;34mName:\x1b[0m \x1b[1;33m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 truncate_string(&self.project_name, 51));
        println!("\x1b[1;34m│\x1b[0m  📂 \x1b[1;34mPath:\x1b[0m \x1b[0;37m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 truncate_string(&self.path, 51));

        let language_icon = match self.language {
            // crate::compiler::ProjectLanguage::Rust => "🦀",
            // crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::Asc => "📜",
            _ => "❓",
        };

        println!("\x1b[1;34m│\x1b[0m  {} \x1b[1;34mLanguage:\x1b[0m \x1b[1;32m{:<49}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 language_icon, format!("{:?}", self.language));

        // if self.is_web_app {
        //     println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;32mWeb Application Detected\x1b[0m                              \x1b[1;34m│\x1b[0m");
        // }

        if !self.build_files.is_empty() {
            println!("\x1b[1;34m│\x1b[0m  🔧 \x1b[1;34mBuild Files:\x1b[0m \x1b[1;33m{:<45}\x1b[0m \x1b[1;34m│\x1b[0m", 
                     self.build_files.join(", "));
        }

        if !self.entry_files.is_empty() {
            println!("\x1b[1;34m│\x1b[0m  📄 \x1b[1;34mEntry Files:\x1b[0m \x1b[1;33m{:<45}\x1b[0m \x1b[1;34m│\x1b[0m", 
                     self.entry_files.join(", "));
        }

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    /// Get a brief summary
    pub fn get_summary(&self) -> String {
        let language_icon = match self.language {
            // crate::compiler::ProjectLanguage::Rust => "🦀",
            // crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::Asc => "📜",
            _ => "❓",
        };

        // let app_type = if self.is_web_app { " (Web App)" } else { "" };

        format!("{} {:?} project{}", language_icon, self.language, "")
    }
}

// Helper functions

fn extract_entry_points(verification: &VerificationResult) -> Vec<String> {
    let mut entry_points = Vec::new();

    // Check for standard entry points
    for export_name in &verification.export_names {
        if is_entry_point(export_name) {
            entry_points.push(export_name.clone());
        }
    }

    // If we have a start section, note that
    if verification.has_start_section {
        if let Some(index) = verification.start_function_index {
            entry_points.push(format!("_start (index {index})"));
        } else {
            entry_points.push("_start".to_string());
        }
    }

    entry_points
}

fn is_entry_point(name: &str) -> bool {
    matches!(
        name,
        "main" | "_start" | "start" | "init" | "run" | "execute" | "_initialize"
    )
}

fn detect_wasm_bindgen(path: &Path) -> bool {
    // Check if there's a corresponding JS file with wasm-bindgen patterns
    if let Some(parent) = path.parent() {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        // Look for corresponding JS file
        let js_candidates = [
            format!("{stem}.js"),
            format!("{}_bg.js", stem.trim_end_matches("_bg")),
        ];

        for js_name in &js_candidates {
            let js_path = parent.join(js_name);
            if js_path.exists() {
                if let Ok(content) = fs::read_to_string(&js_path) {
                    if content.contains("wasm_bindgen") || content.contains("__wbindgen") {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn determine_module_type(
    verification: &Option<VerificationResult>,
    is_wasm_bindgen: bool,
    is_wasi: bool,
) -> ModuleType {
    if is_wasm_bindgen {
        ModuleType::WasmBindgen
    } else if is_wasi {
        ModuleType::WasiModule
    } else if let Some(ref verify_result) = verification {
        if verify_result.valid_magic && verify_result.has_export_section {
            ModuleType::StandardWasm
        } else {
            ModuleType::Unknown
        }
    } else {
        ModuleType::Unknown
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::VerificationResult;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    const VALID_WASM_BYTES: [u8; 8] = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    fn create_wasm_file_with_extension(content: &[u8]) -> tempfile::NamedTempFile {
        let mut temp_file = tempfile::Builder::new().suffix(".wasm").tempfile().unwrap();
        temp_file.write_all(content).unwrap();
        temp_file
    }

    fn create_mock_verification_result() -> VerificationResult {
        VerificationResult {
            valid_magic: true,
            file_size: 100,
            section_count: 3,
            sections: vec![],
            has_export_section: true,
            export_names: vec!["main".to_string(), "test_func".to_string()],
            has_start_section: false,
            start_function_index: None,
            has_memory_section: true,
            memory_limits: Some((1, Some(10))),
            has_table_section: false,
            function_count: 5,
        }
    }

    #[test]
    fn test_module_type_display() {
        assert_eq!(
            format!("{}", ModuleType::StandardWasm),
            "Standard WebAssembly"
        );
        assert_eq!(
            format!("{}", ModuleType::WasmBindgen),
            "WASM-Bindgen Module"
        );
        assert_eq!(format!("{}", ModuleType::WasiModule), "WASI Module");
        assert_eq!(format!("{}", ModuleType::WebApplication), "Web Application");
        assert_eq!(format!("{}", ModuleType::Unknown), "Unknown");
    }

    #[test]
    fn test_extract_entry_points() {
        let verification = VerificationResult {
            export_names: vec!["main".to_string(), "init".to_string(), "other".to_string()],
            has_start_section: true,
            start_function_index: Some(0),
            ..create_mock_verification_result()
        };

        let entry_points = extract_entry_points(&verification);
        assert!(entry_points.contains(&"main".to_string()));
        assert!(entry_points.contains(&"init".to_string()));
        assert!(!entry_points.contains(&"other".to_string()));
        assert!(entry_points.iter().any(|p| p.contains("_start")));
    }

    #[test]
    fn test_is_entry_point() {
        assert!(is_entry_point("main"));
        assert!(is_entry_point("_start"));
        assert!(is_entry_point("start"));
        assert!(is_entry_point("init"));
        assert!(is_entry_point("run"));
        assert!(is_entry_point("execute"));
        assert!(is_entry_point("_initialize"));
        assert!(!is_entry_point("other"));
        assert!(!is_entry_point(""));
    }

    #[test]
    fn test_detect_wasm_bindgen() {
        let temp_dir = tempdir().unwrap();
        let wasm_path = temp_dir.path().join("test.wasm");
        let js_path = temp_dir.path().join("test.js");

        // Create WASM file
        File::create(&wasm_path).unwrap();

        // Create JS file with wasm-bindgen content
        let mut js_file = File::create(&js_path).unwrap();
        js_file
            .write_all(b"import * as wasm_bindgen from './test_bg.wasm';")
            .unwrap();

        let result = detect_wasm_bindgen(&wasm_path);
        assert!(result);
    }

    #[test]
    fn test_detect_wasm_bindgen_no_js() {
        let temp_dir = tempdir().unwrap();
        let wasm_path = temp_dir.path().join("test.wasm");
        File::create(&wasm_path).unwrap();

        let result = detect_wasm_bindgen(&wasm_path);
        assert!(!result);
    }

    #[test]
    fn test_determine_module_type_wasm_bindgen() {
        let verification = Some(create_mock_verification_result());
        let module_type = determine_module_type(&verification, true, false);
        assert!(matches!(module_type, ModuleType::WasmBindgen));
    }

    #[test]
    fn test_determine_module_type_wasi() {
        let verification = Some(create_mock_verification_result());
        let module_type = determine_module_type(&verification, false, true);
        assert!(matches!(module_type, ModuleType::WasiModule));
    }

    #[test]
    fn test_determine_module_type_standard() {
        let verification = Some(create_mock_verification_result());
        let module_type = determine_module_type(&verification, false, false);
        assert!(matches!(module_type, ModuleType::StandardWasm));
    }

    #[test]
    fn test_determine_module_type_unknown() {
        let module_type = determine_module_type(&None, false, false);
        assert!(matches!(module_type, ModuleType::Unknown));
    }

    #[test]
    fn test_truncate_string_short() {
        let result = truncate_string("short", 10);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_string_long() {
        let result = truncate_string("this is a very long string", 10);
        assert_eq!(result, "this is...");
    }

    #[test]
    fn test_truncate_string_exact() {
        let result = truncate_string("exactly10!", 10);
        assert_eq!(result, "exactly10!");
    }

    #[test]
    fn test_wasm_analysis_invalid_file() {
        let temp_file = create_wasm_file_with_extension(&[0x00, 0x00, 0x00, 0x00]);
        let result = WasmAnalysis::analyze(temp_file.path().to_str().unwrap());

        // Should still succeed but with invalid WASM
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(!analysis.is_valid);
    }

    #[test]
    fn test_wasm_analysis_get_summary_invalid() {
        let temp_file = create_wasm_file_with_extension(&[0x00, 0x00, 0x00, 0x00]);
        let analysis = WasmAnalysis::analyze(temp_file.path().to_str().unwrap()).unwrap();

        let summary = analysis.get_summary();
        assert!(summary.contains("❌"));
        assert!(summary.contains("Invalid"));
    }

    #[test]
    fn test_wasm_analysis_get_summary_valid() {
        let temp_file = create_wasm_file_with_extension(&VALID_WASM_BYTES);
        let analysis = WasmAnalysis::analyze(temp_file.path().to_str().unwrap()).unwrap();

        let summary = analysis.get_summary();
        assert!(
            summary.contains("⚡")
                || summary.contains("🔧")
                || summary.contains("🌐")
                || summary.contains("📱")
        );
    }

    #[test]
    fn test_project_analysis_with_rust_project() {
        let temp_dir = tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        let mut file = File::create(&cargo_toml).unwrap();
        file.write_all(b"[package]\nname = \"test\"").unwrap();

        let result = ProjectAnalysis::analyze(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert_eq!(analysis.language, crate::compiler::ProjectLanguage::Rust);
        assert!(analysis.build_files.contains(&"Cargo.toml".to_string()));
        assert!(analysis.has_cargo_toml);
    }

    #[test]
    fn test_project_analysis_with_entry_files() {
        let temp_dir = tempdir().unwrap();

        // Create entry file
        let main_rs = temp_dir.path().join("main.rs");
        File::create(&main_rs).unwrap();

        let result = ProjectAnalysis::analyze(temp_dir.path().to_str().unwrap());
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.entry_files.contains(&"main.rs".to_string()));
    }

    #[test]
    fn test_project_analysis_nonexistent_dir() {
        let result = ProjectAnalysis::analyze("/nonexistent/directory");
        assert!(result.is_err());
    }

    #[test]
    fn test_project_analysis_get_summary() {
        let temp_dir = tempdir().unwrap();
        let analysis = ProjectAnalysis::analyze(temp_dir.path().to_str().unwrap()).unwrap();

        let summary = analysis.get_summary();
        assert!(summary.contains("project"));
        assert!(
            summary.contains("❓")
                || summary.contains("🦀")
                || summary.contains("🐹")
                || summary.contains("🔧")
        );
    }
}
