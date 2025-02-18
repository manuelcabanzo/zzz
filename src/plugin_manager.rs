use std::path::Path;
use crate::plugin_loader::PluginLoader;

pub struct PluginManager {
    loader: PluginLoader,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            loader: PluginLoader::new(),
        }
    }

    pub fn install_plugin(&mut self, plugin_path: &Path) {
        self.loader.load_plugin(plugin_path);
    }

    pub fn uninstall_plugin(&mut self, plugin_name: &str) {
        self.loader.unload_plugin(plugin_name);
    }

    pub fn list_plugins(&self) -> Vec<String> {
        self.loader.list_plugins()
    }
}
