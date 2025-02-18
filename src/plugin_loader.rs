use std::path::Path;
use libloading::{Library, Symbol};
use crate::plugin_interface::Plugin;

pub struct PluginLoader {
    plugins: Vec<Box<dyn Plugin>>,
    libraries: Vec<Library>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            libraries: Vec::new(),
        }
    }

    pub fn load_plugin(&mut self, plugin_path: &Path) {
        unsafe {
            match Library::new(plugin_path) {
                Ok(lib) => {
                    self.libraries.push(lib);
                    let plugin: Symbol<fn() -> Box<dyn Plugin>> = self.libraries.last().unwrap().get(b"create_plugin").expect("Failed to load symbol");
                    let plugin_instance = plugin();
                    plugin_instance.activate();
                    self.plugins.push(plugin_instance);
                }
                Err(e) => {
                    eprintln!("Failed to load plugin: {:?}", e);
                    return;
                }
            }
        }
    }

    pub fn unload_plugin(&mut self, plugin_name: &str) {
        if let Some(index) = self.plugins.iter().position(|p| p.name() == plugin_name) {
            self.plugins[index].deactivate();
            self.plugins.remove(index);
            self.libraries.remove(index);
        }
    }

    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name().to_string()).collect()
    }
}
