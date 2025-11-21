//! Detect and report potential issues in WASM modules

use crate::runtime::core::module::Module;

/// Represents a potential issue found in a WASM module
#[derive(Debug, Clone)]
pub struct WasmIssue {
    pub severity: IssueSeverity,
    pub title: String,
    pub description: String,
}

/// Severity levels for issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

impl IssueSeverity {
    pub fn emoji(&self) -> &'static str {
        match self {
            IssueSeverity::Info => "ℹ️",
            IssueSeverity::Warning => "⚠️",
            IssueSeverity::Error => "❌",
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            IssueSeverity::Info => "\x1b[0;36m",    // Cyan
            IssueSeverity::Warning => "\x1b[0;33m",  // Yellow
            IssueSeverity::Error => "\x1b[0;31m",    // Red
        }
    }
}

/// Analyze a WASM module and detect potential issues
pub fn detect_issues(module: &Module) -> Vec<WasmIssue> {
    let mut issues = Vec::new();

    // Check for missing or unusual sections
    check_section_completeness(module, &mut issues);

    // Check memory configuration
    check_memory_configuration(module, &mut issues);

    // Check for common patterns that might indicate issues
    check_export_patterns(module, &mut issues);

    // Check for suspicious code characteristics
    check_code_characteristics(module, &mut issues);

    // Check for import/export mismatch
    check_import_export_consistency(module, &mut issues);

    // Check for global variable issues
    check_global_variables(module, &mut issues);

    issues
}

/// Check if required sections are present and well-formed
fn check_section_completeness(module: &Module, issues: &mut Vec<WasmIssue>) {
    // Type section should be present for most modules
    if module.types.is_empty() && !module.imports.is_empty() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Warning,
            title: "No type signatures found".to_string(),
            description: "Module has imports but no type section. This is unusual.".to_string(),
        });
    }

    // If there are functions, there should be code
    let function_count = module.functions.len();
    let code_count = module.functions.iter().filter(|f| !f.code.is_empty()).count();

    if function_count > 0 && code_count == 0 {
        issues.push(WasmIssue {
            severity: IssueSeverity::Error,
            title: "Functions without code".to_string(),
            description: format!(
                "Found {} function declarations but no code implementations. Module is likely invalid.",
                function_count
            ),
        });
    }

    // Check if functions have extremely small code (might be stubs)
    let tiny_functions = module
        .functions
        .iter()
        .filter(|f| f.code.len() > 0 && f.code.len() < 3)
        .count();

    if tiny_functions as f64 / code_count as f64 > 0.5 && code_count > 10 {
        issues.push(WasmIssue {
            severity: IssueSeverity::Info,
            title: "Many minimal functions".to_string(),
            description: format!(
                "{} out of {} functions are very small (< 3 bytes). May indicate stub functions.",
                tiny_functions, code_count
            ),
        });
    }
}

/// Check memory configuration for issues
fn check_memory_configuration(module: &Module, issues: &mut Vec<WasmIssue>) {
    match &module.memory {
        Some(mem) => {
            // Check for extremely large initial memory
            if mem.initial > 1000 {
                issues.push(WasmIssue {
                    severity: IssueSeverity::Warning,
                    title: "Unusually large initial memory".to_string(),
                    description: format!(
                        "Initial memory is {} pages ({} MB). This may cause issues on some platforms.",
                        mem.initial,
                        mem.initial / 16
                    ),
                });
            }

            // Check for mismatched max memory
            if let Some(max) = mem.max {
                if max < mem.initial {
                    issues.push(WasmIssue {
                        severity: IssueSeverity::Error,
                        title: "Invalid memory limits".to_string(),
                        description: format!(
                            "Maximum memory ({} pages) is less than initial ({} pages). This is invalid.",
                            max, mem.initial
                        ),
                    });
                }

                // Check for overly restrictive max memory
                if max > 0 && max < 10 && mem.initial > max {
                    issues.push(WasmIssue {
                        severity: IssueSeverity::Warning,
                        title: "Restrictive memory limit".to_string(),
                        description: format!(
                            "Maximum memory is limited to {} pages. Runtime may fail if memory is exhausted.",
                            max
                        ),
                    });
                }
            }
        }
        None => {
            // No memory section
            if !module.data.is_empty() {
                issues.push(WasmIssue {
                    severity: IssueSeverity::Error,
                    title: "Data segments without memory".to_string(),
                    description: "Module has data segments but no memory section. This is invalid."
                        .to_string(),
                });
            }
        }
    }
}

/// Check export patterns for issues
fn check_export_patterns(module: &Module, issues: &mut Vec<WasmIssue>) {
    // No exports at all
    if module.exports.is_empty() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Info,
            title: "No exports found".to_string(),
            description: "Module has no exports. It can only be used internally or as a library."
                .to_string(),
        });
    }

    // Check for suspicious export counts
    if module.exports.len() > 500 {
        issues.push(WasmIssue {
            severity: IssueSeverity::Warning,
            title: "Very large export table".to_string(),
            description: format!(
                "Module exports {} items. This is unusual and may indicate bloat.",
                module.exports.len()
            ),
        });
    }

    // Check if all exports are describe functions (wasm-bindgen pattern)
    let describe_count = module
        .exports
        .iter()
        .filter(|(name, _)| name.contains("describe"))
        .count();

    if describe_count > 0 && describe_count == module.exports.len() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Info,
            title: "Only describe exports".to_string(),
            description: "All exports are wasm-bindgen describe functions. This module appears to be a \
                           wasm-bindgen artifact or build intermediate."
                .to_string(),
        });
    }
}

/// Check code characteristics for issues
fn check_code_characteristics(module: &Module, issues: &mut Vec<WasmIssue>) {
    let total_code_size: usize = module.functions.iter().map(|f| f.code.len()).sum();

    if total_code_size == 0 && !module.functions.is_empty() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Error,
            title: "No function code".to_string(),
            description: "Module declares functions but has no code section. Module is likely corrupt."
                .to_string(),
        });
    }

    // Check for average function size
    if !module.functions.is_empty() {
        let avg_size = total_code_size / module.functions.len();

        if avg_size > 10000 {
            issues.push(WasmIssue {
                severity: IssueSeverity::Warning,
                title: "Very large average function size".to_string(),
                description: format!(
                    "Average function size is {} bytes. Functions may not be optimized.",
                    avg_size
                ),
            });
        }

        // Check for gigantic single function
        if let Some(max_size) = module.functions.iter().map(|f| f.code.len()).max() {
            if max_size > 100000 {
                issues.push(WasmIssue {
                    severity: IssueSeverity::Warning,
                    title: "Extremely large function detected".to_string(),
                    description: format!(
                        "One function is {} bytes. This may indicate a problem with compilation or inlining.",
                        max_size
                    ),
                });
            }
        }
    }
}

/// Check for import/export consistency issues
fn check_import_export_consistency(module: &Module, issues: &mut Vec<WasmIssue>) {
    // Check for imported functions that are never called
    if !module.imports.is_empty() && module.functions.is_empty() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Info,
            title: "Only imports, no internal functions".to_string(),
            description: "Module imports functions but defines no internal functions. \
                           It's a thin wrapper or interface."
                .to_string(),
        });
    }

    // Check for suspicious import counts
    if module.imports.len() > 100 {
        issues.push(WasmIssue {
            severity: IssueSeverity::Warning,
            title: "Large number of imports".to_string(),
            description: format!(
                "Module imports {} items. High dependency count may affect performance.",
                module.imports.len()
            ),
        });
    }
}

/// Check global variable configuration
fn check_global_variables(module: &Module, issues: &mut Vec<WasmIssue>) {
    if module.globals.is_empty() {
        return;
    }

    // Count mutable globals
    let mutable_count = module.globals.iter().filter(|g| g.mutable).count();

    if mutable_count == module.globals.len() {
        issues.push(WasmIssue {
            severity: IssueSeverity::Info,
            title: "All globals are mutable".to_string(),
            description: format!(
                "All {} global variables are mutable. This may indicate less optimized code.",
                module.globals.len()
            ),
        });
    }

    // Check for suspicious global count
    if module.globals.len() > 100 {
        issues.push(WasmIssue {
            severity: IssueSeverity::Warning,
            title: "Large number of globals".to_string(),
            description: format!(
                "Module defines {} global variables. This is unusual and may affect performance.",
                module.globals.len()
            ),
        });
    }
}

/// Display detected issues in a formatted way
pub fn display_issues(issues: &[WasmIssue]) {
    if issues.is_empty() {
        println!("  ✅ \x1b[1;32mNo significant issues detected\x1b[0m");
        return;
    }

    // Sort by severity (errors first, then warnings, then info)
    let mut sorted_issues = issues.to_vec();
    sorted_issues.sort_by_key(|i| std::cmp::Reverse(i.severity));

    println!("  🔍 \x1b[1;34mDetected Issues:\x1b[0m");

    for issue in sorted_issues {
        let color = issue.severity.color_code();
        let reset = "\x1b[0m";
        println!("     {} {}{}{}:",
            issue.severity.emoji(),
            color,
            issue.title,
            reset
        );
        println!("        {}", issue.description);
    }
}
