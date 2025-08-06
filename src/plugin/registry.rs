//! Plugin registry for managing plugin instances

use crate::error::Result;
use crate::plugin::Plugin;

pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        self.plugins.push(plugin);
        Ok(())
    }

    pub fn into_plugins(self) -> Vec<Box<dyn Plugin>> {
        self.plugins
    }

    pub fn get_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    pub fn find_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        for plugin in &self.plugins {
            if plugin.info().name == name {
                return Some(plugin.as_ref());
            }
        }
        None
    }

    pub fn clear(&mut self) {
        self.plugins.clear();
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}
