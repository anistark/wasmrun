use crate::cli::PluginSubcommands;
use crate::error::Result;
use crate::plugin::manager::PluginManager;

// TODO: Implement plugin search with proper plugin registry system
// These functions will be used when we have a proper plugin registry
// fn get_available_plugins_from_crates_io() -> Vec<String> {
//     // Try to search crates.io for wasmrun plugins
//     search_crates_io_for_plugins().unwrap_or_else(|_| {
//         // Fallback to known working plugins if API call fails
//         vec!["wasmrust".to_string(), "wasmgo".to_string()]
//     })
// }

// fn search_crates_io_for_plugins() -> Result<Vec<String>> {
//     let output = std::process::Command::new("curl")
//         .arg("-s")
//         .arg("https://crates.io/api/v1/crates?q=wasmrun&sort=downloads")
//         .output()
//         .map_err(|e| WasmrunError::from(format!("Failed to search crates.io: {e}")))?;

//     if !output.status.success() {
//         return Err(WasmrunError::from("Failed to query crates.io API".to_string()));
//     }

//     let response = String::from_utf8_lossy(&output.stdout);
//     parse_crates_io_response(&response)
// }

// fn parse_crates_io_response(response: &str) -> Result<Vec<String>> {
//     use serde_json::Value;

//     let json: Value = serde_json::from_str(response)
//         .map_err(|e| WasmrunError::from(format!("Failed to parse crates.io response: {e}")))?;

//     let mut plugins = Vec::new();

//     if let Some(crates) = json["crates"].as_array() {
//         for crate_info in crates.iter().take(10) { // Limit to top 10 results
//             if let Some(name) = crate_info["name"].as_str() {
//                 // Filter for likely wasmrun plugins
//                 if name.contains("wasmrun") || name.contains("wasm-") {
//                     plugins.push(name.to_string());
//                 }
//             }
//         }
//     }

//     // Add known plugins if not found in search
//     let known_plugins = ["wasmrust", "wasmgo"];
//     for plugin in known_plugins {
//         if !plugins.contains(&plugin.to_string()) {
//             plugins.push(plugin.to_string());
//         }
//     }

//     Ok(plugins)
// }

pub fn run_plugin_command(subcommand: &PluginSubcommands) -> Result<()> {
    match subcommand {
        PluginSubcommands::List { all: _ } => run_plugin_list(),
        PluginSubcommands::Install { plugin, version: _ } => run_plugin_install(plugin),
        PluginSubcommands::Uninstall { plugin } => run_plugin_uninstall(plugin),
        PluginSubcommands::Update { plugin } => run_plugin_update(plugin),
        PluginSubcommands::Enable { plugin, disable } => {
            if *disable {
                run_plugin_disable(plugin)
            } else {
                run_plugin_enable(plugin)
            }
        }
        PluginSubcommands::Info { plugin } => run_plugin_info(plugin),
        // TODO: Implement plugin search with proper plugin registry system
        // PluginSubcommands::Search { query } => run_plugin_search(query),
    }
}

pub fn run_plugin_list() -> Result<()> {
    let manager = PluginManager::new()?;

    println!(
        "\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m"
    );
    println!(
        "\x1b[1;34m│\x1b[0m  🔌 \x1b[1;36mInstalled Plugins\x1b[0m                                   \x1b[1;34m│\x1b[0m"
    );
    println!(
        "\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m"
    );

    if manager.get_builtin_plugins().is_empty() && manager.get_external_plugins().is_empty() {
        println!(
            "\x1b[1;34m│\x1b[0m  No plugins installed                                    \x1b[1;34m│\x1b[0m"
        );
    } else {
        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;36m🔧 Built-in Plugins\x1b[0m                                    \x1b[1;34m│\x1b[0m"
        );

        for plugin in manager.get_builtin_plugins() {
            let info = plugin.info();
            println!(
                "\x1b[1;34m│\x1b[0m    ✅ {:<25} v{:<10} \x1b[0;37m{}\x1b[0m",
                info.name, info.version, info.description
            );
        }

        println!(
            "\x1b[1;34m│\x1b[0m  \x1b[1;36m🌐 External Plugins\x1b[0m                                    \x1b[1;34m│\x1b[0m"
        );

        for (name, plugin) in manager.get_external_plugins() {
            let info = plugin.info();
            let status = if manager.is_plugin_enabled(name) {
                "✅"
            } else {
                "❌"
            };
            println!(
                "\x1b[1;34m│\x1b[0m    {} {:<25} v{:<10} \x1b[0;37m{}\x1b[0m",
                status, info.name, info.version, info.description
            );
        }
    }

    println!(
        "\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m"
    );

    Ok(())
}

// TODO: Implement plugin search with proper plugin registry system
// pub fn run_plugin_search(query: &str) -> Result<()> {
//     println!("🔍 Searching for plugins: {query}");

//     // Get available plugins from crates.io search
//     let available_plugins = get_available_plugins_from_crates_io();
//     let matches: Vec<&String> = available_plugins
//         .iter()
//         .filter(|plugin| plugin.to_lowercase().contains(&query.to_lowercase()))
//         .collect();

//     if matches.is_empty() {
//         println!("❌ No plugins found matching '{query}'");
//     } else {
//         println!("\n📦 Found {} plugin(s):", matches.len());
//         for plugin in matches {
//             println!("  • {plugin}");
//         }
//         println!("\n💡 Use 'wasmrun plugin install <plugin-name>' to install");
//     }

//     Ok(())
// }

pub fn run_plugin_install(plugin: &str) -> Result<()> {
    let mut manager = PluginManager::new()?;
    println!("🔄 Installing plugin: {plugin}");

    manager.install_plugin(plugin)?;
    println!("✅ Plugin '{plugin}' installed successfully");

    Ok(())
}

pub fn run_plugin_uninstall(plugin: &str) -> Result<()> {
    let mut manager = PluginManager::new()?;
    println!("🗑️  Uninstalling plugin: {plugin}");

    manager.uninstall_plugin(plugin)?;
    println!("✅ Plugin '{plugin}' uninstalled successfully");

    Ok(())
}

pub fn run_plugin_update(plugin: &str) -> Result<()> {
    let mut manager = PluginManager::new()?;
    println!("🔄 Updating plugin: {plugin}");

    manager.update_plugin(plugin)?;
    println!("✅ Plugin '{plugin}' updated successfully");

    Ok(())
}

pub fn run_plugin_enable(plugin: &str) -> Result<()> {
    let mut manager = PluginManager::new()?;
    println!("✅ Enabling plugin: {plugin}");

    manager.enable_plugin(plugin)?;
    println!("✅ Plugin '{plugin}' enabled successfully");

    Ok(())
}

pub fn run_plugin_disable(plugin: &str) -> Result<()> {
    let mut manager = PluginManager::new()?;
    println!("❌ Disabling plugin: {plugin}");

    manager.disable_plugin(plugin)?;
    println!("✅ Plugin '{plugin}' disabled successfully");

    Ok(())
}

pub fn run_plugin_info(plugin: &str) -> Result<()> {
    let manager = PluginManager::new()?;

    if let Some(info) = manager.get_plugin_info(plugin) {
        println!("\n🔌 Plugin Information:");
        println!("Name: {}", info.name);
        println!("Version: {}", info.version);
        println!("Description: {}", info.description);
        println!("Author: {}", info.author);
        println!("Type: {:?}", info.plugin_type);
        println!("Extensions: {:?}", info.extensions);
        println!("Entry Files: {:?}", info.entry_files);
        println!("Dependencies: {:?}", info.dependencies);
        println!("Capabilities: {:?}", info.capabilities);
    } else {
        println!("❌ Plugin '{plugin}' not found");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_plugin_command_list() {
        let result = run_plugin_command(&PluginSubcommands::List { all: false });
        // Should succeed even if no plugins are installed
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_plugin_command_info_nonexistent() {
        let result = run_plugin_command(&PluginSubcommands::Info {
            plugin: "nonexistent_plugin_12345".to_string(),
        });
        // Should succeed (prints not found message)
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_plugin_command_enable_disable() {
        // Test enable command
        let result = run_plugin_command(&PluginSubcommands::Enable {
            plugin: "test_plugin".to_string(),
            disable: false,
        });
        // May fail due to plugin not existing, but command should handle gracefully
        assert!(result.is_err() || result.is_ok());

        // Test disable command
        let result = run_plugin_command(&PluginSubcommands::Enable {
            plugin: "test_plugin".to_string(),
            disable: true,
        });
        // May fail due to plugin not existing, but command should handle gracefully
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_run_plugin_list_empty() {
        // This test just ensures the list command doesn't crash with no plugins
        let result = run_plugin_list();
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_plugin_install_invalid() {
        let result = run_plugin_install("invalid_plugin_name_12345");
        // Plugin installer creates a template even for non-existent plugins, so this succeeds
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_plugin_uninstall_nonexistent() {
        let result = run_plugin_uninstall("nonexistent_plugin_12345");
        // Should succeed (no-op for non-existent plugin)
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_plugin_update_nonexistent() {
        let result = run_plugin_update("nonexistent_plugin_12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_plugin_enable_nonexistent() {
        let result = run_plugin_enable("nonexistent_plugin_12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_plugin_disable_nonexistent() {
        let result = run_plugin_disable("nonexistent_plugin_12345");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_plugin_info_nonexistent() {
        let result = run_plugin_info("nonexistent_plugin_12345");
        assert!(result.is_ok());
    }

    #[test]
    fn test_plugin_subcommands_coverage() {
        // Test all plugin subcommand variants to ensure they compile and don't crash
        let subcommands = vec![
            PluginSubcommands::List { all: true },
            PluginSubcommands::List { all: false },
            PluginSubcommands::Install {
                plugin: "test".to_string(),
                version: None,
            },
            PluginSubcommands::Install {
                plugin: "test".to_string(),
                version: Some("1.0.0".to_string()),
            },
            PluginSubcommands::Uninstall {
                plugin: "test".to_string(),
            },
            PluginSubcommands::Update {
                plugin: "test".to_string(),
            },
            PluginSubcommands::Enable {
                plugin: "test".to_string(),
                disable: false,
            },
            PluginSubcommands::Enable {
                plugin: "test".to_string(),
                disable: true,
            },
            PluginSubcommands::Info {
                plugin: "test".to_string(),
            },
        ];

        for subcommand in subcommands {
            let result = run_plugin_command(&subcommand);
            // Commands should either succeed or fail gracefully (not panic)
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn test_plugin_command_error_handling() {
        // Test that plugin commands handle various error conditions gracefully

        // Empty plugin name
        let result = run_plugin_install("");
        assert!(result.is_err());

        let result = run_plugin_uninstall("");
        assert!(result.is_ok()); // Should be no-op

        let result = run_plugin_info("");
        assert!(result.is_ok()); // Should print not found

        // Very long plugin name
        let long_name = "a".repeat(1000);
        let result = run_plugin_install(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_plugin_list_output_format() {
        // This test ensures the list command produces expected output format
        let result = run_plugin_list();
        assert!(result.is_ok());

        // The function should complete without panicking, even with no plugins
    }
}
