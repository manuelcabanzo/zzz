use std::path::Path;
use libloading::{Library, Symbol};
use crate::plugin_interface::Plugin;

pub struct PluginLoader {
    plugins: Vec<*mut dyn Plugin>,
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
                    let plugin: Symbol<fn() -> *mut dyn Plugin> = match lib.get(b"create_plugin") {
                        Ok(symbol) => symbol,
                        Err(e) => {
                            eprintln!("Failed to load symbol 'create_plugin': {:?}", e);
                            return;
                        }
                    };
                    let plugin_instance = plugin();
                    if plugin_instance.is_null() {
                        eprintln!("Failed to create plugin instance");
                        return;
                    }
                    (*plugin_instance).activate();
                    self.plugins.push(plugin_instance);
                    self.libraries.push(lib);
                }
                Err(e) => {
                    eprintln!("Failed to load plugin library: {:?}", e);
                    return;
                }
            }
        }
    }

    pub fn unload_plugin(&mut self, plugin_name: &str) {
        if let Some(index) = self.plugins.iter().position(|&p| unsafe { (*p).name() == plugin_name }) {
            unsafe {
                (*self.plugins[index]).deactivate();
                let _ = Box::from_raw(self.plugins[index]);
            }
            self.plugins.remove(index);
            self.libraries.remove(index);
        }
    }

    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.iter().map(|&p| unsafe { (*p).name().to_string() }).collect()
    }
}
