use std::path::Path;
use libloading::{Library, Symbol};
use crate::plugin_interface::Plugin;
use std::collections::HashMap;

type PluginCreate = fn() -> Box<dyn Plugin>;

pub struct PluginLoader {
    plugins: HashMap<String, Box<dyn Plugin>>,
    libraries: HashMap<String, Library>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            libraries: HashMap::new(),
        }
    }

    pub fn load_plugin(&mut self, plugin_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            println!("Attempting to load library from path: {:?}", plugin_path);
            let library = Library::new(plugin_path)?;
            println!("Library loaded successfully.");

            // Get the create function
            let create_fn: Symbol<PluginCreate> = library.get(b"create_plugin")?;
            println!("Create function loaded successfully.");

            // Create the plugin
            let plugin = create_fn();
            let plugin_name = plugin.name().to_string();
            println!("Plugin created successfully with name: {}", plugin_name);

            // Activate the plugin
            plugin.activate();
            println!("Plugin activated successfully.");

            // Store the plugin and library
            self.plugins.insert(plugin_name.clone(), plugin);
            self.libraries.insert(plugin_name, library);
            println!("Plugin and library stored successfully.");

            Ok(())
        }
    }

    pub fn unload_plugin(&mut self, plugin_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(plugin) = self.plugins.remove(plugin_name) {
            plugin.deactivate();
            self.libraries.remove(plugin_name);
        }
        Ok(())
    }

    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn get_plugin(&self, name: &str) -> Option<&Box<dyn Plugin>> {
        self.plugins.get(name)
    }
}

impl Drop for PluginLoader {
    fn drop(&mut self) {
        // Deactivate all plugins before unloading
        for (_, plugin) in self.plugins.drain() {
            plugin.deactivate();
        }
        // Libraries will be automatically unloaded when dropped
        self.libraries.clear();
    }
}