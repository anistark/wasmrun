use crate::cli::PluginSubcommands;
use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;
use crate::plugin::installer::PluginInstaller;

pub fn handle_plugin_command(plugin_cmd: &PluginSubcommands) -> Result<()> {
    match plugin_cmd {
        PluginSubcommands::List { all } => run_plugin_list(*all),
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
        PluginSubcommands::Search { query } => run_plugin_search(query),
    }
}

pub fn run_plugin_list(_show_all: bool) -> Result<()> {
    let manager = PluginManager::new()?;

    println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
    println!("\x1b[1;34m│\x1b[0m  🔌 \x1b[1;36mWasmrun Plugins\x1b[0m                                      \x1b[1;34m│\x1b[0m");
    println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");

    let stats = manager.get_stats();
    println!("\x1b[1;34m│\x1b[0m  📊 \x1b[1;34mSummary:\x1b[0m {} built-in, {} external, {} enabled      \x1b[1;34m│\x1b[0m", 
             stats.builtin_count, stats.external_count, stats.enabled_count);
    println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");

    println!("\x1b[1;34m│\x1b[0m  \x1b[1;35m📦 Built-in Plugins\x1b[0m                                     \x1b[1;34m│\x1b[0m");
    
    for plugin in manager.get_builtin_plugins() {
        let info = plugin.info();
        println!("\x1b[1;34m│\x1b[0m    \x1b[1;32m✅ {:<25}\x1b[0m v{:<10} \x1b[0;37m{}\x1b[0m", 
                 info.name, info.version, info.description);
    }

    if stats.external_count > 0 {
        println!("\x1b[1;34m│\x1b[0m                                                                 \x1b[1;34m│\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  \x1b[1;36m🌐 External Plugins\x1b[0m                                    \x1b[1;34m│\x1b[0m");
        
        for (name, plugin) in manager.get_external_plugins() {
            let info = plugin.info();
            let status = if manager.is_plugin_enabled(name) { "✅" } else { "❌" };
            println!("\x1b[1;34m│\x1b[0m    {} {:<25} v{:<10} \x1b[0;37m{}\x1b[0m", 
                     status, info.name, info.version, info.description);
        }
    }

    println!("\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m\n");
    
    Ok(())
}

pub fn run_plugin_install(plugin: &str) -> Result<()> {
    println!("Installing plugin: {}", plugin);
    
    // Check if plugin is already installed
    let manager = PluginManager::new()?;
    if manager.is_plugin_installed(plugin) {
        println!("Plugin '{}' is already installed", plugin);
        return Ok(());
    }

    // Step 1: Install the plugin library and setup directory
    println!("📦 Setting up plugin library files...");
    let install_result = PluginInstaller::install_external_plugin(plugin)?;
    let plugin_dir = PluginInstaller::setup_plugin_directory(plugin)?;
    
    println!("✅ Plugin files installed to: {}", plugin_dir.display());

    // Step 2: Register plugin in wasmrun config
    println!("📋 Registering plugin with wasmrun...");
    let mut manager = PluginManager::new()?;
    
    match manager.register_installed_plugin(plugin) {
        Ok(()) => {
            println!("✅ Plugin '{}' registered successfully", plugin);
        }
        Err(e) => {
            println!("⚠️  Plugin files installed but registration had issues: {}", e);
            println!("   Plugin may still be functional for basic operations");
        }
    }

    // Step 3: Verify installation
    println!("🔍 Verifying installation...");
    let verification = PluginInstaller::verify_plugin_installation(plugin)?;
    
    if verification.is_functional {
        println!("🎉 Plugin '{}' installed and verified successfully!", plugin);
        
        // Show installation summary
        println!("\n📋 Installation Summary:");
        println!("   Plugin: {}", plugin);
        if !install_result.version.is_empty() {
            println!("   Version: {}", install_result.version);
        }
        println!("   Location: {}", plugin_dir.display());
        println!("   Status: Functional ✅");
        
        // Show basic usage info
        match plugin {
            "wasmrust" => {
                println!("\n💡 Usage:");
                println!("   wasmrun compile ./rust-project");
                println!("   wasmrun ./rust-project --watch");
            }
            "wasmgo" => {
                println!("\n💡 Usage:");
                println!("   wasmrun compile ./go-project");
                println!("   wasmrun ./go-project --watch");
            }
            _ => {}
        }
    } else {
        println!("⚠️  Plugin '{}' installed but may have issues:", plugin);
        
        if !verification.binary_available {
            println!("   - Plugin library files not properly installed");
        }
        if !verification.dependencies_available {
            println!("   - Dependencies missing:");
            match plugin {
                "wasmrust" => {
                    println!("     • Install: rustup target add wasm32-unknown-unknown");
                    println!("     • Ensure cargo and rustc are available");
                }
                "wasmgo" => {
                    println!("     • Install TinyGo: https://tinygo.org/getting-started/install/");
                }
                _ => {}
            }
        }
        if !verification.directory_exists {
            println!("   - Plugin directory structure invalid");
        }
        
        println!("\n🔧 Try fixing the issues above and run:");
        println!("   wasmrun plugin install {}", plugin);
    }

    Ok(())
}

pub fn run_plugin_uninstall(plugin: &str) -> Result<()> {
    println!("Uninstalling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    
    // Check if plugin is installed
    if !manager.is_plugin_installed(plugin) {
        println!("Plugin '{}' is not installed", plugin);
        return Ok(());
    }
    
    // Remove from wasmrun config
    println!("📋 Removing plugin from wasmrun configuration...");
    manager.uninstall_plugin(plugin)?;
    
    // Remove plugin directory
    println!("🗂️  Removing plugin directory...");
    PluginInstaller::remove_plugin_directory(plugin)?;
    
    // Uninstall library
    if should_uninstall_binary() {
        println!("🧹 Cleaning up plugin library files...");
        match PluginInstaller::uninstall_plugin_library(plugin) {
            Ok(()) => println!("Plugin library '{}' also removed", plugin),
            Err(e) => println!("Warning: Could not remove plugin library: {}", e),
        }
    }
    
    println!("✅ Plugin '{}' uninstalled successfully", plugin);
    Ok(())
}

pub fn run_plugin_update(plugin: &str) -> Result<()> {
    println!("Updating plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.update_plugin(plugin)?;
    
    println!("Plugin '{}' updated successfully", plugin);
    Ok(())
}

pub fn run_plugin_enable(plugin: &str) -> Result<()> {
    println!("Enabling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.enable_plugin(plugin)?;
    
    println!("Plugin '{}' enabled", plugin);
    Ok(())
}

pub fn run_plugin_disable(plugin: &str) -> Result<()> {
    println!("Disabling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.disable_plugin(plugin)?;
    
    println!("Plugin '{}' disabled", plugin);
    Ok(())
}

pub fn run_plugin_info(plugin: &str) -> Result<()> {
    let manager = PluginManager::new()?;
    
    if let Some(info) = manager.get_plugin_info(plugin) {
        println!("\n\x1b[1;34m╭─────────────────────────────────────────────────────────────────╮\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  🔌 \x1b[1;36mPlugin Information: {}\x1b[0m", info.name);
        println!("\x1b[1;34m├─────────────────────────────────────────────────────────────────┤\x1b[0m");
        println!("\x1b[1;34m│\x1b[0m  📦 \x1b[1;34mName:\x1b[0m {:<49} \x1b[1;34m│\x1b[0m", info.name);
        println!("\x1b[1;34m│\x1b[0m  🏷️  \x1b[1;34mVersion:\x1b[0m {:<44} \x1b[1;34m│\x1b[0m", info.version);
        println!("\x1b[1;34m│\x1b[0m  📝 \x1b[1;34mDescription:\x1b[0m {:<40} \x1b[1;34m│\x1b[0m", info.description);
        
        if let Some(source_info) = manager.get_plugin_source_info(plugin) {
            println!("\x1b[1;34m│\x1b[0m  🌐 \x1b[1;34mSource:\x1b[0m {:<44} \x1b[1;34m│\x1b[0m", source_info);
        }
        
        let health = manager.check_plugin_health(plugin)?;
        match health {
            crate::plugin::manager::PluginHealthStatus::Healthy => {
                println!("\x1b[1;34m│\x1b[0m  ✅ \x1b[1;34mStatus:\x1b[0m \x1b[1;32mHealthy\x1b[0m                              \x1b[1;34m│\x1b[0m");
            }
            crate::plugin::manager::PluginHealthStatus::MissingDependencies(deps) => {
                println!("\x1b[1;34m│\x1b[0m  ⚠️  \x1b[1;34mStatus:\x1b[0m \x1b[1;33mMissing dependencies\x1b[0m                  \x1b[1;34m│\x1b[0m");
                for dep in deps {
                    println!("\x1b[1;34m│\x1b[0m    - {:<51} \x1b[1;34m│\x1b[0m", dep);
                }
            }
            crate::plugin::manager::PluginHealthStatus::NotFound => {
                println!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;34mStatus:\x1b[0m \x1b[1;31mNot found\x1b[0m                             \x1b[1;34m│\x1b[0m");
            }
            crate::plugin::manager::PluginHealthStatus::LoadError(err) => {
                println!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;34mStatus:\x1b[0m \x1b[1;31mLoad error: {}\x1b[0m", err);
            }
        }
        
        println!("\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m\n");
    } else {
        return Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)));
    }
    
    Ok(())
}

pub fn run_plugin_search(_query: &str) -> Result<()> {
    println!("Plugin search not yet implemented");
    Ok(())
}

fn should_uninstall_binary() -> bool {
    println!("Do you also want to remove the plugin library files? (y/N)");
    
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim().to_lowercase();
        matches!(input.as_str(), "y" | "yes")
    } else {
        false
    }
}
