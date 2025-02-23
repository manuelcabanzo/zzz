use std::path::Path;
use libloading::{Library, Symbol};
use crate::plugin_exports::{Plugin, PluginWrapper};
use std::collections::HashMap;
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};

type PluginCreate = unsafe fn() -> *mut PluginWrapper;

enum PluginMessage {
    Error(String),
    Success(String, Box<dyn Plugin>, Library),
}

pub struct PluginLoader {
    plugins: HashMap<String, Box<dyn Plugin>>,
    libraries: HashMap<String, Library>,
    message_sender: Sender<PluginMessage>,
    message_receiver: Receiver<PluginMessage>,
}

impl PluginLoader {
    pub fn new() -> Self {
        let (message_sender, message_receiver) = mpsc::channel();
        Self {
            plugins: HashMap::new(),
            libraries: HashMap::new(),
            message_sender,
            message_receiver,
        }
    }

    pub fn load_plugin(&mut self, plugin_path: &Path) {
        let plugin_path = plugin_path.to_path_buf();
        let sender = self.message_sender.clone();

        thread::spawn(move || {
            unsafe {
                println!("Attempting to load library from path: {:?}", plugin_path);
                let library = match Library::new(&plugin_path) {
                    Ok(lib) => lib,
                    Err(e) => {
                        let _ = sender.send(PluginMessage::Error(format!("Failed to load library: {}", e)));
                        return;
                    }
                };
                
                println!("Loading create_plugin symbol");
                let create_fn: Symbol<PluginCreate> = match library.get(b"create_plugin\0") {
                    Ok(symbol) => symbol,
                    Err(e) => {
                        let _ = sender.send(PluginMessage::Error(format!("Failed to load create_plugin symbol: {}", e)));
                        return;
                    }
                };
                
                println!("Creating plugin instance");
                let raw_wrapper = create_fn();
                if raw_wrapper.is_null() {
                    let _ = sender.send(PluginMessage::Error("Failed to create plugin instance: returned null pointer".to_string()));
                    return;
                }

                // Convert the raw pointer back to a Box
                let wrapper = Box::from_raw(raw_wrapper);
                let plugin_name = wrapper.name().to_string();
                let plugin: Box<dyn Plugin> = Box::new(*wrapper);
                
                println!("Activating plugin");
                plugin.activate();
                
                println!("Storing plugin");
                let _ = sender.send(PluginMessage::Success(plugin_name, plugin, library));
            }
        });

        // Process any pending messages
        while let Ok(message) = self.message_receiver.try_recv() {
            match message {
                PluginMessage::Error(error) => {
                    eprintln!("Plugin loading error: {}", error);
                }
                PluginMessage::Success(name, plugin, library) => {
                    self.plugins.insert(name.clone(), plugin);
                    self.libraries.insert(name, library);
                }
            }
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

    pub fn check_errors(&mut self) -> Option<String> {
        if let Ok(message) = self.message_receiver.try_recv() {
            match message {
                PluginMessage::Error(error) => Some(error),
                PluginMessage::Success(name, plugin, library) => {
                    self.plugins.insert(name.clone(), plugin);
                    self.libraries.insert(name, library);
                    None
                }
            }
        } else {
            None
        }
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