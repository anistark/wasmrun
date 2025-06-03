use crate::commands::{verify_wasm, VerificationResult};
use crate::error::{ChakraError, Result};
use crate::utils::PathResolver;
use std::fs;
use std::path::Path;

/// Comprehensive WASM file analysis for CLI display
#[derive(Debug)]
pub struct WasmAnalysis {
    pub path: String,
    pub filename: String,
    pub file_size: String,
    pub file_size_bytes: u64,
    #[allow(dead_code)]
    pub verification: Option<VerificationResult>,
    pub is_valid: bool,
    pub entry_points: Vec<String>,
    pub is_wasm_bindgen: bool,
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
    /// Perform comprehensive analysis of a WASM file
    pub fn analyze(path: &str) -> Result<Self> {
        let path_obj = Path::new(path);

        // Validate file exists and has correct extension
        PathResolver::validate_wasm_file(path)?;

        let filename = PathResolver::get_filename(path)?;
        let file_size_bytes = fs::metadata(path)
            .map_err(|e| ChakraError::add_context(format!("Getting file size for {}", path), e))?
            .len();

        let file_size = format_file_size(file_size_bytes);

        // Perform verification analysis
        let verification = match verify_wasm(path) {
            Ok(result) => Some(result),
            Err(_) => None,
        };

        let is_valid = verification.as_ref().map_or(false, |v| v.valid_magic);

        // Analyze entry points
        let entry_points = if let Some(ref verify_result) = verification {
            extract_entry_points(verify_result)
        } else {
            Vec::new()
        };

        // Determine module characteristics
        let is_wasm_bindgen = detect_wasm_bindgen(path_obj);
        let is_wasi = verification.as_ref().map_or(false, |v| {
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

        // Basic file info
        println!(
            "\x1b[1;34m│\x1b[1;34mFile:\x1b[0m \x1b[1;33m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m",
            truncate_string(&self.filename, 51)
        );
        println!(
            "\x1b[1;34m│\x1b[1;34mSize:\x1b[0m \x1b[1;33m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m",
            format!("{} ({} bytes)", self.file_size, self.file_size_bytes)
        );
        println!(
            "\x1b[1;34m│\x1b[1;34mPath:\x1b[0m \x1b[0;37m{:<51}\x1b[0m \x1b[1;34m│\x1b[0m",
            truncate_string(&self.path, 51)
        );

        // Module type and validity
        let validity_icon = if self.is_valid { "✅" } else { "❌" };
        let validity_text = if self.is_valid {
            "Valid WebAssembly"
        } else {
            "Invalid Format"
        };
        println!("\x1b[1;34m│\x1b[0m  {} \x1b[1;34mFormat:\x1b[0m \x1b[1;32m{:<49}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 validity_icon, validity_text);

        println!("\x1b[1;34m│\x1b[0m  🏷️  \x1b[1;34mType:\x1b[0m \x1b[1;36m{:<53}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 self.module_type.to_string());

        // Module statistics
        if self.is_valid {
            println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");
            println!("\x1b[1;34m│\x1b[0m  📦 \x1b[1;36mModule Statistics\x1b[0m                                      \x1b[1;34m│\x1b[0m");
            println!("\x1b[1;34m│\x1b[0m     \x1b[1;34mExports:\x1b[0m \x1b[1;33m{:<47}\x1b[0m \x1b[1;34m│\x1b[0m", self.exports_count);
            println!("\x1b[1;34m│\x1b[0m     \x1b[1;34mFunctions:\x1b[0m \x1b[1;33m{:<45}\x1b[0m \x1b[1;34m│\x1b[0m", self.functions_count);

            // Entry points
            if !self.entry_points.is_empty() {
                println!("\x1b[1;34m│\x1b[0m     \x1b[1;34mEntry Points:\x1b[0m \x1b[1;32m{:<43}\x1b[0m \x1b[1;34m│\x1b[0m", 
                         self.entry_points.join(", "));
            } else {
                println!("\x1b[1;34m│\x1b[0m     \x1b[1;34mEntry Points:\x1b[0m \x1b[1;33m{:<43}\x1b[0m \x1b[1;34m│\x1b[0m", 
                         "None detected");
            }

            // Special characteristics
            if self.is_wasi {
                println!("\x1b[1;34m│\x1b[0m     \x1b[1;32m✓ WASI Support Detected\x1b[0m                               \x1b[1;34m│\x1b[0m");
            }
            if self.is_wasm_bindgen {
                println!("\x1b[1;34m│\x1b[0m     \x1b[1;32m✓ WASM-Bindgen Module\x1b[0m                                 \x1b[1;34m│\x1b[0m");
            }
        }

        println!(
            "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
        );
    }

    /// Get a brief one-line summary for compact display
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
    pub is_web_app: bool,
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
        let is_web_app = language == crate::compiler::ProjectLanguage::Rust
            && crate::compiler::is_rust_web_application(path);

        // Detect important files
        let mut entry_files = Vec::new();
        let mut build_files = Vec::new();
        let mut has_cargo_toml = false;

        // Check for common files
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
            is_web_app,
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
            crate::compiler::ProjectLanguage::Rust => "🦀",
            crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::AssemblyScript => "📜",
            crate::compiler::ProjectLanguage::Python => "🐍",
            _ => "❓",
        };

        println!("\x1b[1;34m│\x1b[0m  {} \x1b[1;34mLanguage:\x1b[0m \x1b[1;32m{:<49}\x1b[0m \x1b[1;34m│\x1b[0m", 
                 language_icon, format!("{:?}", self.language));

        if self.is_web_app {
            println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;32mWeb Application Detected\x1b[0m                              \x1b[1;34m│\x1b[0m");
        }

        // Show important files
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
            crate::compiler::ProjectLanguage::Rust => "🦀",
            crate::compiler::ProjectLanguage::Go => "🐹",
            crate::compiler::ProjectLanguage::C => "🔧",
            crate::compiler::ProjectLanguage::AssemblyScript => "📜",
            crate::compiler::ProjectLanguage::Python => "🐍",
            _ => "❓",
        };

        let app_type = if self.is_web_app { " (Web App)" } else { "" };

        format!("{} {:?} project{}", language_icon, self.language, app_type)
    }
}

// Helper functions

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} bytes", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

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
            entry_points.push(format!("_start (index {})", index));
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
            format!("{}.js", stem),
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
