use eframe::egui;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::file_system::FileSystem;
use rfd::FileDialog;

use crate::components::{
    file_panel::FilePanel,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
};

pub struct IDE {
    file_panel: FilePanel,
    code_editor: CodeEditor,
    console_panel: ConsolePanel,
    emulator_panel: EmulatorPanel,
    settings_modal: SettingsModal,
    file_system: Option<Rc<FileSystem>>,
    project_path: Option<PathBuf>,
    show_file_panel: bool,
    show_console_panel: bool,
    show_emulator_panel: bool,
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ide = Self {
            file_panel: FilePanel::new(),
            code_editor: CodeEditor::new(),
            console_panel: ConsolePanel::new(),
            emulator_panel: EmulatorPanel::new(),
            settings_modal: SettingsModal::new(),
            file_system: None,
            project_path: None,
            show_file_panel: false,
            show_console_panel: false,
            show_emulator_panel: false,
        };
        ide.settings_modal.apply_theme(&cc.egui_ctx);
        ide
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl {
                self.show_file_panel = !self.show_file_panel;
            }
            if i.key_pressed(egui::Key::Num2) && i.modifiers.ctrl {
                self.show_emulator_panel = !self.show_emulator_panel;
            }
            if i.key_pressed(egui::Key::Num3) && i.modifiers.ctrl {
                self.show_console_panel = !self.show_console_panel;
            }
            if i.key_pressed(egui::Key::M) && i.modifiers.ctrl {
                self.settings_modal.show_settings_menu = !self.settings_modal.show_settings_menu;
            }
            if i.key_pressed(egui::Key::O) && i.modifiers.ctrl {
                self.open_folder();
            }
        });
    }

    fn open_folder(&mut self) {
        if let Some(folder_path) = FileDialog::new().pick_folder() {
            self.project_path = Some(folder_path.clone());
            self.file_system = Some(Rc::new(FileSystem::new(folder_path.to_str().unwrap())));
            self.file_panel.project_path = Some(folder_path.clone());
            self.file_panel.file_system = self.file_system.clone();
            self.file_panel.expanded_folders.clear();
            self.file_panel.expanded_folders.insert(folder_path.clone());
            self.console_panel.log(&format!("Opened project: {}", folder_path.display()));
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard_shortcuts(ctx);

        if self.show_file_panel {
            self.file_panel.show(ctx, &mut self.code_editor.code, &mut self.code_editor.current_file, &mut |msg| self.console_panel.log(msg));
        }

        if self.show_emulator_panel {
            self.emulator_panel.show(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {    
            let fixed_editor_height = 730.0;    
            let available_height = if self.show_console_panel {         
                (ui.available_height() - 280.0).min(fixed_editor_height)    
            } else {        
                fixed_editor_height    
            };    
            self.code_editor.show(ui, available_height);
        });

        if self.show_console_panel {
            self.console_panel.show(ctx);
        }

        self.settings_modal.show(ctx);
    }
}

impl eframe::App for IDE {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update(ctx, frame);
    }
}
