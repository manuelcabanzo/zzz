pub mod core {
    pub mod ide;
    pub mod file_system;
    pub mod terminal;
    pub mod lsp_server;
    pub mod lsp_client;
}

pub mod utils {
    pub mod themes;
}

pub mod components {
    pub mod file_panel;
    pub mod code_editor;
    pub mod console_panel;
    pub mod emulator_panel;
    pub mod settings_modal;
    pub mod ui {
        mod modal;
        mod context_menu;

        pub use modal::Modal;
        pub use context_menu::context_menu;
    }
}
