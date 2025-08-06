use crate::cli::PluginSubcommands;
use crate::error::{Result, WasmrunError};
use crate::plugin::manager::PluginManager;

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
    println!("🔌 Installing plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.install_plugin(plugin)?;
    
    println!("✅ Plugin '{}' installed successfully", plugin);
    Ok(())
}

pub fn run_plugin_uninstall(plugin: &str) -> Result<()> {
    println!("🗑️  Uninstalling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.uninstall_plugin(plugin)?;
    
    println!("✅ Plugin '{}' uninstalled successfully", plugin);
    Ok(())
}

pub fn run_plugin_update(plugin: &str) -> Result<()> {
    println!("🔄 Updating plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.update_plugin(plugin)?;
    
    println!("✅ Plugin '{}' updated successfully", plugin);
    Ok(())
}

pub fn run_plugin_enable(plugin: &str) -> Result<()> {
    println!("🔌 Enabling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.enable_plugin(plugin)?;
    
    println!("✅ Plugin '{}' enabled", plugin);
    Ok(())
}

pub fn run_plugin_disable(plugin: &str) -> Result<()> {
    println!("❌ Disabling plugin: {}", plugin);
    
    let mut manager = PluginManager::new()?;
    manager.disable_plugin(plugin)?;
    
    println!("✅ Plugin '{}' disabled", plugin);
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
                println!("\x1b[1;34m│\x1b[0m  ✅ \x1b[1;34mStatus:\x1b[0m \x1b[1;32mHealthy\x1b[0m                               \x1b[1;34m│\x1b[0m");
            }
            crate::plugin::manager::PluginHealthStatus::MissingDependencies(deps) => {
                println!("\x1b[1;34m│\x1b[0m  ⚠️  \x1b[1;34mStatus:\x1b[0m \x1b[1;33mMissing Dependencies\x1b[0m                \x1b[1;34m│\x1b[0m");
                for dep in deps {
                    println!("\x1b[1;34m│\x1b[0m     \x1b[1;31m• {}\x1b[0m", dep);
                }
            }
            crate::plugin::manager::PluginHealthStatus::NotFound => {
                println!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;34mStatus:\x1b[0m \x1b[1;31mNot Found\x1b[0m                             \x1b[1;34m│\x1b[0m");
            }
            crate::plugin::manager::PluginHealthStatus::LoadError(err) => {
                println!("\x1b[1;34m│\x1b[0m  ❌ \x1b[1;34mStatus:\x1b[0m \x1b[1;31mLoad Error: {}\x1b[0m", err);
            }
        }
        
        println!("\x1b[1;34m╰─────────────────────────────────────────────────────────────────╯\x1b[0m\n");
    } else {
        return Err(WasmrunError::from(format!("Plugin '{}' not found", plugin)));
    }
    
    Ok(())
}

pub fn run_plugin_search(query: &str) -> Result<()> {
    println!("🔍 Searching for plugins matching: {}", query);
    println!("Plugin search functionality not yet implemented");
    Ok(())
}
