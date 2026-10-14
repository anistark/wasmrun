use crate::config::ExternalPluginEntry;
use crate::error::Result;

/// crates.io as the plugin source: whether a name exists there, and the
/// entry recorded for one being installed.
pub struct PluginRegistry;

impl PluginRegistry {
    /// Validates if a plugin exists and is installable
    pub fn validate_plugin(plugin_name: &str) -> Result<bool> {
        if plugin_name.is_empty() {
            return Ok(false);
        }
        Ok(crate::utils::SystemUtils::get_latest_crates_version(plugin_name).is_some())
    }

    /// Creates a plugin entry
    pub fn create_plugin_entry(plugin_name: &str) -> Result<ExternalPluginEntry> {
        use crate::plugin::external::ExternalPluginLoader;
        ExternalPluginLoader::create_generic_entry(plugin_name)
    }
}
