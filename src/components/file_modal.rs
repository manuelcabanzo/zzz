use eframe::egui;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use std::collections::HashSet;
use rfd::FileDialog;
use crate::core::file_system::FileSystem;

pub struct FileModal {
    pub show: bool,
    pub file_system: Option<Rc<FileSystem>>,
    pub project_path: Option<PathBuf>,
    pub expanded_folders: HashSet<PathBuf>,
    pub rename_dialog: Option<(PathBuf, String)>,
    pub selected_folder: Option<PathBuf>,
    show_delete_confirmation: Option<PathBuf>,
    selected_item: Option<PathBuf>,  // New field to track the selected item
}

impl FileModal {
    pub fn new() -> Self {
        Self {
            show: false,
            file_system: None,
            project_path: None,
            expanded_folders: HashSet::new(),
            rename_dialog: None,
            selected_folder: None,
            show_delete_confirmation: None,
            selected_item: None,  // Initialize the new field
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, code: &mut String, current_file: &mut Option<String>, log: &mut dyn FnMut(&str)) {
        if !self.show {
            return;
        }

        let modal_size = egui::vec2(500.0, 500.0);
        egui::Window::new("File Browser")
            .fixed_size(modal_size)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_size(modal_size);
                
                ui.vertical(|ui| {
                    ui.heading("Files");

                    ui.horizontal(|ui| {
                        if ui.button("Open Folder").clicked() {
                            self.open_project(log);
                        }
                        if self.file_system.is_some() {
                            if ui.button("New File").clicked() {
                                self.create_new_item(false, log);
                            }
                            if ui.button("New Folder").clicked() {
                                self.create_new_item(true, log);
                            }
                            if ui.button("Save").clicked() {
                                self.save_current_file(code, current_file, log);
                            }
                            if ui.button("Delete").clicked() {
                                if let Some(selected_path) = &self.selected_item {
                                    // Show confirmation dialog for the selected item
                                    self.show_delete_confirmation = Some(selected_path.clone());
                                } else {
                                    log("No file or folder selected for deletion.");
                                }
                            }
                        }
                    });
                    
                    if let Some(path_to_delete) = self.show_delete_confirmation.clone() {
                        egui::Window::new("Confirm Deletion")
                            .collapsible(false)
                            .resizable(false)
                            .show(ctx, |ui| {
                                ui.label(format!("Are you sure you want to delete '{}'? This action cannot be undone.", path_to_delete.display()));

                                ui.horizontal(|ui| {
                                    if ui.button("Yes").clicked() {
                                        // Delete the file or folder
                                        if let Some(file_system) = &self.file_system {
                                            match file_system.delete_file(&path_to_delete) {
                                                Ok(_) => {
                                                    log(&format!("Deleted: {}", path_to_delete.display()));
                                                    self.selected_item = None;  // Clear the selection after deletion
                                                },
                                                Err(e) => log(&format!("Failed to delete {}: {:?}", path_to_delete.display(), e)),
                                            }
                                        }
                                        self.show_delete_confirmation = None; // Close the confirmation window
                                    }

                                    if ui.button("No").clicked() {
                                        self.show_delete_confirmation = None; // Close without deleting
                                    }
                                });
                            });
                    }

                    self.show_rename_dialog(ctx, code, current_file, log);
                    ui.separator();

                    if let (Some(fs), Some(project_path)) = (&self.file_system, &self.project_path) {
                        let mut expanded_folders = self.expanded_folders.clone();
                        let mut rename_dialog = self.rename_dialog.clone();
                        let mut selected_folder = self.selected_folder.clone();
                        let mut log_messages = Vec::new();

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            Self::render_folder_contents(
                                ui, ctx, project_path, fs, &mut expanded_folders, code,
                                current_file, &mut |msg: &str| log_messages.push(msg.to_string()),
                                &mut rename_dialog, &mut selected_folder, 0,
                                &mut self.selected_item,  // Pass the selected_item
                            );
                        });

                        self.expanded_folders = expanded_folders;
                        self.rename_dialog = rename_dialog;
                        self.selected_folder = selected_folder;

                        for msg in log_messages {
                            log(&msg);
                        }
                    } else {
                        ui.label("No project opened. Click 'Open Folder' to start.");
                    }
                });
            });
    }

    pub fn selected_file_or_folder(&self) -> Option<PathBuf> {
        // Logic to return the currently selected file or folder
        // For example, if using a variable to track the selected item:
        self.project_path.clone()
    }

    fn open_project(&mut self, log: &mut dyn FnMut(&str)) {
        if let Some(path) = FileDialog::new().pick_folder() {
            self.project_path = Some(path.clone());
            self.file_system = Some(Rc::new(FileSystem::new(path.to_str().unwrap())));
            self.expanded_folders.clear();
            self.expanded_folders.insert(path.clone());
            log(&format!("Opened project: {}", path.display()));
        }
    }

    fn show_rename_dialog(&mut self, ctx: &egui::Context, code: &mut String, current_file: &mut Option<String>, log: &mut dyn FnMut(&str)) {
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
                action = Some((path.clone(), new_path.clone(), old_name.clone(), new_name));
                log(&format!("Confirmation clicked. Old path: {}, New path: {}", path.display(), new_path.display()));
            } else if canceled {
                log("Rename dialog canceled");
                self.rename_dialog = None;
            }
        }

        if let Some((old_path, new_path, old_name, new_name)) = action {
            log(&format!("Attempting to create/rename: {} to {}", old_path.display(), new_path.display()));

            if let Some(fs) = &self.file_system {
                if old_path.exists() {
                    match fs.rename_file(&old_path, &new_path) {
                        Ok(_) => {
                            log(&format!("Renamed '{}' to '{}'", old_name, new_name));
                            if let Some(current_file_path) = current_file {
                                if current_file_path == old_path.to_str().unwrap() {
                                    *current_file = Some(new_path.to_str().unwrap().to_string());
                                }
                            }
                        }
                        Err(e) => log(&format!("Error renaming: {}", e)),
                    }
                } else {
                    let is_folder = !new_path.extension().is_some();
                    if is_folder {
                        match fs.create_directory(&new_path) {
                            Ok(_) => log(&format!("Created new folder: {}", new_path.display())),
                            Err(e) => log(&format!("Error creating folder: {}", e)),
                        }
                    } else {
                        match fs.create_new_file(new_path.parent().unwrap(), &new_name) {
                            Ok(_) => {
                                *current_file = Some(new_path.to_str().unwrap().to_string());
                                code.clear();
                                log(&format!("Created new file: {}", new_path.display()));
                            }
                            Err(e) => log(&format!("Error creating file: {}", e)),
                        }
                    }
                }

                if fs.path_exists(&new_path) {
                    log(&format!("Confirmed: {} exists", new_path.display()));
                } else {
                    log(&format!("Warning: {} does not exist after creation attempt", new_path.display()));
                }

                if let Some(parent) = new_path.parent() {
                    self.expanded_folders.insert(parent.to_path_buf());
                }
            } else {
                log("Error: File system not initialized");
            }
            self.rename_dialog = None;
        }
    }

    fn create_new_item(&mut self, is_folder: bool, log: &mut dyn FnMut(&str)) {
        log(&format!("Creating new {}", if is_folder { "folder" } else { "file" }));

        if let Some(fs) = &self.file_system {
            let parent_folder = if let Some(selected_folder) = &self.selected_folder {
                selected_folder.clone()
            } else {
                fs.get_project_directory().to_path_buf()
            };

            log(&format!("Parent folder: {}", parent_folder.display()));

            let default_name = if is_folder {
                format!("new_folder_{}", chrono::Utc::now().timestamp())
            } else {
                format!("new_file_{}.txt", chrono::Utc::now().timestamp())
            };

            log(&format!("Default name: {}", default_name));

            let new_path = parent_folder.join(&default_name);
            self.rename_dialog = Some((new_path.clone(), default_name.clone()));

            log(&format!("Rename dialog set for new {} at path: {}", 
                if is_folder { "folder" } else { "file" }, new_path.display()
            ));
        } else {
            log("Error: File system not initialized");
        }
    }

    fn save_current_file(&self, code: &str, current_file: &Option<String>, log: &mut dyn FnMut(&str)) {
        if let Some(file) = current_file {
            if let Some(fs) = &self.file_system {
                let path = Path::new(file);
                match fs.save_file(path, code) {
                    Ok(_) => log(&format!("Saved file: {}", file)),
                    Err(e) => log(&format!("Error saving file {}: {}", file, e)),
                }
            }
        } else {
            log("No file is currently open.");
        }
    }

    fn render_folder_contents(
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        folder: &Path,
        fs: &Rc<FileSystem>,
        expanded_folders: &mut HashSet<PathBuf>,
        code: &mut String,
        current_file: &mut Option<String>,
        log: &mut dyn FnMut(&str),
        rename_dialog: &mut Option<(PathBuf, String)>,
        selected_folder: &mut Option<PathBuf>,
        indent_level: usize,
        selected_item: &mut Option<PathBuf>,
    ) {
        if let Ok(entries) = fs.list_directory(folder) {
            for entry in entries {
                let path = folder.join(&entry.name);
                let is_dir = entry.is_dir;
                let is_expanded = expanded_folders.contains(&path);
                let _id = ui.make_persistent_id(&path);

                ui.horizontal(|ui| {
                    ui.add_space(indent_level as f32 * 20.0);

                    let text = if is_dir {
                        format!("{}", entry.name)
                    } else {
                        format!(" {}", entry.name)
                    };

                    let is_selected = selected_item.as_ref() == Some(&path);
                    let text_color = if is_selected {
                        egui::Color32::from_rgb(100, 100, 255)
                    } else {
                        ui.style().visuals.text_color()
                    };

                    let label = if is_dir {
                        egui::RichText::new(text).italics().color(text_color)
                    } else {
                        egui::RichText::new(text).color(text_color)
                    };

                    let response = ui.add(egui::Label::new(label).sense(egui::Sense::click()));

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if response.clicked() {
                        *selected_item = Some(path.clone());  // Update the selected item
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

                    if response.hovered() {
                        let hover_text = if is_dir {
                            "Click to expand/collapse and select"
                        } else {
                            "Click to open file and select"
                        };
                        response.on_hover_text(hover_text);
                    }
                });

                if is_dir && is_expanded {
                    Self::render_folder_contents(
                        ui, ctx, &path, fs, expanded_folders, code,
                        current_file, log, rename_dialog, selected_folder, indent_level + 1,
                        selected_item,
                    );
                }
            }
        } else {
            log(&format!("Error reading directory: {}", folder.display()));
        }
    }
}
