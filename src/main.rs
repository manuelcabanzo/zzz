use eframe::egui;
mod file_system;
use file_system::FileSystem;
use std::path::{PathBuf, Path};
use rfd::FileDialog;
use std::rc::Rc;
use std::collections::HashSet;

struct IDE {
    code: String,
    console_output: String,
    show_file_panel: bool,
    show_console_panel: bool,
    show_emulator_panel: bool,
    file_system: Option<Rc<FileSystem>>,
    current_file: Option<String>,
    project_path: Option<PathBuf>,
    expanded_folders: HashSet<PathBuf>,
    rename_dialog: Option<(PathBuf, String)>,
    selected_folder: Option<PathBuf>,
}

impl Default for IDE {
    fn default() -> Self {
        Self {
            code: String::new(),
            console_output: String::new(),
            show_file_panel: false,
            show_console_panel: false,
            show_emulator_panel: false,
            file_system: None,
            current_file: None,
            project_path: None,
            expanded_folders: HashSet::new(),
            rename_dialog: None,
            selected_folder: None,
        }
    }
}

impl IDE {


fn log(&mut self, message: &str) {
    self.console_output.push_str(message);
    self.console_output.push('\n');
    println!("{}", message);
}

    fn open_project(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            self.project_path = Some(path.clone());
            self.file_system = Some(Rc::new(FileSystem::new(path.to_str().unwrap())));
            self.expanded_folders.clear();
            self.expanded_folders.insert(path.clone());
            self.log(&format!("Opened project: {}", path.display()));
        }
    }

    fn render_folder_contents(
        ui: &mut egui::Ui,
        folder: &Path,
        fs: &Rc<FileSystem>,
        expanded_folders: &mut HashSet<PathBuf>,
        code: &mut String,
        current_file: &mut Option<String>,
        log: &mut dyn FnMut(&str),
        rename_dialog: &mut Option<(PathBuf, String)>,
        selected_folder: &mut Option<PathBuf>,
    ) {
        if let Ok(entries) = fs.list_directory(folder) {
            for entry in entries {
                let path = folder.join(&entry.name);
                let is_dir = entry.is_dir;
                let is_expanded = expanded_folders.contains(&path);
                let id = ui.make_persistent_id(&path);
                let text = if is_dir {
                    format!("📁 {}", entry.name)
                } else {
                    format!("📄 {}", entry.name)
                };
                let is_selected = selected_folder.as_ref().map_or(false, |sf| sf == &path);

                ui.horizontal(|ui| {
                    let response = if is_dir {
                        let header = egui::CollapsingHeader::new(text)
                            .id_source(id);

                        let state = header.show(ui, |ui| {
                            if is_expanded {
                                Self::render_folder_contents(
                                    ui,
                                    &path,
                                    fs,
                                    expanded_folders,
                                    code,
                                    current_file,
                                    log,
                                    rename_dialog,
                                    selected_folder,
                                );
                            }
                        });

                        state.header_response.clone()
                    } else {
                        ui.label(text)
                    };

                    if is_selected {
                        response.clone().highlight();
                    }

                    if response.clicked() {
                        if is_dir {
                            if is_expanded {
                                expanded_folders.remove(&path);
                            } else {
                                expanded_folders.insert(path.clone());
                            }
                            *selected_folder = Some(path.clone());
                        } else {
                            match fs.open_file(&path) {
                                Ok(content) => {
                                    *code = content;
                                    *current_file = Some(path.to_str().unwrap().to_string());
                                    log(&format!("Opened file: {}", path.display()));
                                }
                                Err(e) => log(&format!("Error opening file {}: {}", path.display(), e)),
                            }
                        }
                    }

                    if ui.button("🖊").on_hover_text("Rename").clicked() {
                        *rename_dialog = Some((path.clone(), entry.name.clone()));
                    }
                    if ui.button("🗑").on_hover_text("Delete").clicked() {
                        if let Err(e) = fs.delete_file(&path) {
                            log(&format!("Error deleting {}: {}", path.display(), e));
                        } else {
                            log(&format!("Deleted {}: {}", if is_dir { "folder" } else { "file" }, path.display()));
                            if !is_dir && current_file.as_ref().map(|f| f == path.to_str().unwrap()).unwrap_or(false) {
                                *current_file = None;
                                *code = String::new();
                            }
                            expanded_folders.remove(&path);
                        }
                    }
                });
            }
        } else {
            log(&format!("Error reading directory: {}", folder.display()));
        }
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
        });
    }

    fn show_file_panel(&mut self, ctx: &egui::Context) {
        if self.show_file_panel {
            egui::SidePanel::left("file_panel")
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.heading("Files");
                    ui.horizontal(|ui| {
                        if ui.button("Open Folder").clicked() {
                            self.open_project();
                        }
                        if self.file_system.is_some() {
                            if ui.button("New File").clicked() {
                                self.create_new_item(false);
                            }
                            if ui.button("New Folder").clicked() {
                                self.create_new_item(true);
                            }
                            if ui.button("Save").clicked() {
                                self.save_current_file();
                            }
                        }
                    });
                    ui.separator();
                    if let (Some(fs), Some(project_path)) = (&self.file_system, &self.project_path) {
                        let mut expanded_folders = self.expanded_folders.clone();
                        let mut code = self.code.clone();
                        let mut current_file = self.current_file.clone();
                        let mut rename_dialog = self.rename_dialog.clone();
                        let mut selected_folder = self.selected_folder.clone();
                        let mut log_messages = Vec::new();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            Self::render_folder_contents(
                                ui,
                                project_path,
                                fs,
                                &mut expanded_folders,
                                &mut code,
                                &mut current_file,
                                &mut |msg: &str| log_messages.push(msg.to_string()),
                                &mut rename_dialog,
                                &mut selected_folder,
                            );
                        });
                        self.expanded_folders = expanded_folders;
                        self.code = code;
                        self.current_file = current_file;
                        self.rename_dialog = rename_dialog;
                        self.selected_folder = selected_folder;
                        for msg in log_messages {
                            self.log(&msg);
                        }
                    } else {
                        ui.label("No project opened. Click 'Open Folder' to start.");
                    }
                });
        }
    }

    fn create_new_item(&mut self, is_folder: bool) {
        if let Some(fs) = &self.file_system {
            let parent_folder = if let Some(selected_folder) = &self.selected_folder {
                selected_folder.clone()
            } else {
                fs.get_project_directory().to_path_buf()
            };

            let default_name = if is_folder {
                format!("new_folder_{}", chrono::Utc::now().timestamp())
            } else {
                format!("new_file_{}.txt", chrono::Utc::now().timestamp())
            };

            self.rename_dialog = Some((parent_folder.join(&default_name), default_name));
            self.log(&format!("Enter name for new {}", if is_folder { "folder" } else { "file" }));
        }
    }

    fn save_current_file(&mut self) {
        if let Some(file) = &self.current_file {
            if let Some(fs) = &self.file_system {
                let path = Path::new(file);
                match fs.save_file(path, &self.code) {
                    Ok(_) => self.log(&format!("Saved file: {}", file)),
                    Err(e) => self.log(&format!("Error saving file {}: {}", file, e)),
                }
            }
        } else {
            self.log("No file is currently open.");
        }
    }

    fn show_emulator_panel(&self, ctx: &egui::Context) {
        if self.show_emulator_panel {
            egui::SidePanel::right("emulator_panel")
                .resizable(false)
                .exact_width(500.0)
                .show(ctx, |ui| {
                    ui.heading("Emulator");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Emulator goes here");
                    });
                });
        }
    }


    fn show_code_editor(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Code Editor");
            if let Some(file) = &self.current_file {
                ui.label(format!("Editing: {}", file));
            }
            let available_height = if self.show_console_panel {
                ui.available_height() - 250.0 // Subtracting console panel height
            } else {
                ui.available_height() - 30.0 // Only subtracting space for heading
            };
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let text_edit = egui::TextEdit::multiline(&mut self.code)
                        .desired_width(f32::INFINITY);
                    let frame = egui::Frame::none().inner_margin(4.0);
                    frame.show(ui, |ui| {
                        ui.add_sized([ui.available_width(), available_height], text_edit);
                    });
                });
        });
    }

fn show_console_panel(&mut self, ctx: &egui::Context) {
    if self.show_console_panel {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(false)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.heading("Console");
                let scroll_area = egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true);
                scroll_area.show(ui, |ui| {
                    let lines: Vec<_> = self.console_output.lines().collect();
                    let text = lines.join("\n");
                    ui.label(text);
                });
            });
    }
}

     fn show_rename_dialog(&mut self, ctx: &egui::Context) {
        let mut action = None;

        if let Some((path, old_name)) = &mut self.rename_dialog {
            let mut new_name = old_name.clone();
            let mut confirmed = false;
            let mut canceled = false;

            egui::Window::new("Rename")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("New name:");
                        let response = ui.text_edit_singleline(&mut new_name);
                        if response.changed() {
                            *old_name = new_name.clone();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            canceled = true;
                        }
                        if ui.button("Confirm").clicked() {
                            confirmed = true;
                        }
                    });
                });

            if confirmed {
                let new_path = path.with_file_name(&new_name);
                action = Some((path.clone(), new_path, old_name.clone(), new_name));
            } else if canceled {
                action = Some((path.clone(), path.clone(), old_name.clone(), old_name.clone()));
            }
        }

        if let Some((old_path, new_path, old_name, new_name)) = action {
            if old_path != new_path {
                if let Some(fs) = &self.file_system {
                    if old_path.exists() {
                        match fs.rename_file(&old_path, &new_path) {
                            Ok(_) => {
                                self.log(&format!("Renamed '{}' to '{}'", old_name, new_name));
                                if let Some(current_file) = &mut self.current_file {
                                    if current_file == old_path.to_str().unwrap() {
                                        *current_file = new_path.to_str().unwrap().to_string();
                                    }
                                }
                            }
                            Err(e) => self.log(&format!("Error renaming: {}", e)),
                        }
                    } else {
                        let is_folder = !new_path.extension().is_some();
                        if is_folder {
                            match fs.create_directory(&new_path) {
                                Ok(_) => self.log(&format!("Created new folder: {}", new_path.display())),
                                Err(e) => self.log(&format!("Error creating folder: {}", e)),
                            }
                        } else {
                            match fs.create_new_file(new_path.parent().unwrap(), &new_name) {
                                Ok(_) => {
                                    self.current_file = Some(new_path.to_str().unwrap().to_string());
                                    self.code.clear();
                                    self.log(&format!("Created new file: {}", new_path.display()));
                                }

                                Err(e) => self.log(&format!("Error creating file: {}", e)),
                            }
                        }
                    }
                    if let Some(parent) = new_path.parent() {
                        self.expanded_folders.insert(parent.to_path_buf());
                    }
                }
            }
            self.rename_dialog = None;
        }
    }
}

impl eframe::App for IDE {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard_shortcuts(ctx);
        self.show_file_panel(ctx);
        self.show_rename_dialog(ctx);
        self.show_emulator_panel(ctx);
        self.show_code_editor(ctx);
        self.show_console_panel(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "ZZZ IDE",
        options,
        Box::new(|_cc| Ok(Box::new(IDE::default()))),
    )
}

