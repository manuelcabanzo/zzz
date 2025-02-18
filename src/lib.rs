pub mod core {
    pub mod ide;
    pub mod file_system;
    pub mod terminal;
    pub mod app_state;
    pub mod git_manager;
    pub mod search;
    pub mod constants;
    pub mod app_creation;
    pub mod downloader;
    pub mod android_resources;
}

pub mod utils {
    pub mod themes;
}

pub mod components {
    pub mod file_modal;
    pub mod code_editor;
    pub mod console_panel;
    pub mod emulator_panel;
    pub mod settings_modal;
    pub mod ai_assistant;
    pub mod git_modal;
}

pub mod plugin_manager;
pub mod plugin_loader;
pub mod plugin_interface;
