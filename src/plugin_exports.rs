use std::any::Any;

// The C-compatible wrapper struct
#[repr(C)]
#[derive(Clone)]
pub struct PluginWrapper {
    name: String,
    version: String,
    activated: bool,
}

impl PluginWrapper {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            activated: false,
        }
    }
}

// The main Plugin trait
pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn activate(&self);
    fn deactivate(&self);
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Plugin>;

    // Optional hooks with default implementations
    fn on_file_operation(&self) {}
    fn on_editor_update(&self, _buffer: &str) {}
    fn on_console_update(&self) {}
    fn on_git_operation(&self) {}
}

// Implement Clone for boxed plugins
impl Clone for Box<dyn Plugin> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// Implement Plugin trait for PluginWrapper
impl Plugin for PluginWrapper {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn activate(&self) {
        println!("Activating plugin: {}", self.name);
    }

    fn deactivate(&self) {
        println!("Deactivating plugin: {}", self.name);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Plugin> {
        Box::new(self.clone())
    }
}
