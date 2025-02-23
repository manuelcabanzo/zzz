use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::plugin_exports::Plugin;
use crate::plugin_loader::PluginLoader;

pub struct PluginManager {
    loader: Arc<Mutex<PluginLoader>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            loader: Arc::new(Mutex::new(PluginLoader::new())),
        }
    }

    pub fn install_plugin(&self, plugin_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        println!("Installing plugin from path: {:?}", plugin_path);
        let mut loader = self.loader.lock().unwrap();
        loader.load_plugin(plugin_path);
        Ok(())
    }

    pub fn uninstall_plugin(&self, plugin_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut loader = self.loader.lock().unwrap();
        loader.unload_plugin(plugin_name)
    }

    pub fn list_plugins(&self) -> Vec<String> {
        let loader = self.loader.lock().unwrap();
        loader.list_plugins()
    }

    pub fn get_plugin(&self, name: &str) -> Option<Arc<Box<dyn Plugin>>> {
        let loader = self.loader.lock().unwrap();
        loader.get_plugin(name).map(|plugin| Arc::new(plugin.clone_box()))
    }

    pub fn load_plugin(&self, plugin_path: &Path) {
        let mut loader = self.loader.lock().unwrap();
        loader.load_plugin(plugin_path);
    }

    pub fn check_errors(&self) -> Option<String> {
        let mut loader = self.loader.lock().unwrap();
        loader.check_errors()
    }
}