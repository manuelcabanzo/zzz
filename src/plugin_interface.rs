use std::any::Any;

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn activate(&self);
    fn deactivate(&self);
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Plugin>;

    // New event hooks
    fn on_file_operation(&self) {} // Default empty implementation
    fn on_editor_update(&self, _buffer: &str) {} // Default empty implementation
    fn on_console_update(&self) {} // Default empty implementation
    fn on_git_operation(&self) {} // Default empty implementation
    
    // You can add more hooks as needed:
    fn on_build(&self) {}
    fn on_debug(&self) {}
    fn on_emulator_start(&self) {}
    fn on_settings_change(&self) {}
}

impl Clone for Box<dyn Plugin> {
    fn clone(&self) -> Box<dyn Plugin> {
        self.clone_box()
    }
}