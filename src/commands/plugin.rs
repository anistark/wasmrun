use crate::cli::PluginSubcommands;
use crate::error::Result;
use crate::plugin::manager::PluginCommands;

pub fn handle_plugin_command(cmd: &PluginSubcommands) -> Result<()> {
    match cmd {
        PluginSubcommands::List { all } => {
            let commands = PluginCommands::new()?;
            commands.list(*all)
        }

        PluginSubcommands::Install { plugin, version } => {
            let mut commands = PluginCommands::new()?;
            commands.install_with_version(plugin, version.as_deref())
        }

        PluginSubcommands::Uninstall { plugin } => {
            let mut commands = PluginCommands::new()?;
            commands.uninstall(plugin)
        }

        PluginSubcommands::Update { plugin } => {
            let mut commands = PluginCommands::new()?;
            if plugin == "all" {
                commands.update_all()
            } else {
                commands.update(plugin)
            }
        }

        PluginSubcommands::Enable { plugin, disable } => {
            let mut commands = PluginCommands::new()?;
            if *disable {
                commands.disable(plugin)
            } else {
                commands.enable(plugin)
            }
        }

        PluginSubcommands::Info { plugin } => {
            let commands = PluginCommands::new()?;
            commands.info(plugin)
        }

        PluginSubcommands::Search { query } => {
            let commands = PluginCommands::new()?;
            commands.search(query)
        }
    }
}
