use std::path::Path;
use libloading::{Library, Symbol};
use crate::plugin_exports::Plugin;
use std::collections::HashMap;
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};

type PluginCreate = unsafe fn() -> *mut dyn Plugin;

pub struct PluginLoader {
    plugins: HashMap<String, Box<dyn Plugin>>,
    libraries: HashMap<String, Library>,
    error_sender: Sender<String>,
    error_receiver: Receiver<String>,
}

impl PluginLoader {
    pub fn new() -> Self {
        let (error_sender, error_receiver) = mpsc::channel();
        Self {
            plugins: HashMap::new(),
            libraries: HashMap::new(),
            error_sender,
            error_receiver,
        }
    }

    pub fn load_plugin(&mut self, plugin_path: &Path) {
        let plugin_path = plugin_path.to_path_buf();
        let error_sender = self.error_sender.clone();
        thread::spawn(move || {
            unsafe {
                println!("Attempting to load library from path: {:?}", plugin_path);
                let library = match Library::new(&plugin_path) {
                    Ok(lib) => lib,
                    Err(e) => {
                        let _ = error_sender.send(format!("Failed to load library: {}", e));
                        return;
                    }
                };
                
                println!("Loading create_plugin symbol");
                let create_fn: Symbol<PluginCreate> = match library.get(b"create_plugin") {
                    Ok(symbol) => symbol,
                    Err(e) => {
                        let _ = error_sender.send(format!("Failed to load create_plugin symbol: {}", e));
                        return;
                    }
                };
                
                println!("Creating plugin instance");
                let raw_plugin = create_fn();
                if raw_plugin.is_null() {
                    let _ = error_sender.send("Failed to create plugin instance: returned null pointer".to_string());
                    return;
                }
                let plugin = Box::from_raw(raw_plugin);
                
                println!("Activating plugin");
                plugin.activate();
                
                println!("Storing plugin");
                // Send the plugin and library back to the main thread
                let _ = error_sender.send("Plugin loaded successfully".to_string());
            }
        });
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

    pub fn check_errors(&self) -> Option<String> {
        self.error_receiver.try_recv().ok()
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