use crate::cli::PluginSubcommands;
use crate::error::Result;
use crate::plugin::manager::PluginManager;

fn get_available_plugins_from_crates_io() -> Vec<&'static str> {
    // This could be enhanced to actually search crates.io for plugins
    // For now, return known working plugins
    vec!["wasmrust", "wasmgo", "wasmzig", "wasmjs"]
}

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
        PluginSubcommands::Search { query } => run_plugin_search(query),
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

pub fn run_plugin_search(query: &str) -> Result<()> {
    println!("🔍 Searching for plugins: {query}");

    // Basic search implementation - can be enhanced later
    // Get available plugins from crates.io search
    let available_plugins = get_available_plugins_from_crates_io();
    let matches: Vec<&str> = available_plugins
        .iter()
        .filter(|plugin| plugin.to_lowercase().contains(&query.to_lowercase()))
        .copied()
        .collect();

    if matches.is_empty() {
        println!("❌ No plugins found matching '{query}'");
    } else {
        println!("\n📦 Found {} plugin(s):", matches.len());
        for plugin in matches {
            println!("  • {plugin}");
        }
        println!("\n💡 Use 'wasmrun plugin install <plugin-name>' to install");
    }

    Ok(())
}

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
