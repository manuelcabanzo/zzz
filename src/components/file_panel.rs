use eframe::egui;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use std::collections::HashSet;
use rfd::FileDialog;
use crate::core::file_system::FileSystem;

pub struct FilePanel {
    pub file_system: Option<Rc<FileSystem>>,
    pub project_path: Option<PathBuf>,
    pub expanded_folders: HashSet<PathBuf>,
    pub rename_dialog: Option<(PathBuf, String)>,
    pub selected_folder: Option<PathBuf>,
}

impl FilePanel {
    pub fn new() -> Self {
        Self {
            file_system: None,
            project_path: None,
            expanded_folders: HashSet::new(),
            rename_dialog: None,
            selected_folder: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, code: &mut String, current_file: &mut Option<String>, log: &mut dyn FnMut(&str)) {
        egui::SidePanel::left("file_panel")
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
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
                    }
                });
                
                self.show_rename_dialog(ctx, code, current_file, log);

                ui.separator();
                if let (Some(fs), Some(project_path)) = (&self.file_system, &self.project_path) {
                    let mut expanded_folders = self.expanded_folders.clone();
                    let mut rename_dialog = self.rename_dialog.clone();
                    let mut selected_folder = self.selected_folder.clone();
                    let mut log_messages = Vec::new();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        Self::render_folder_contents(
                            ui,
                            ctx,
                            project_path,
                            fs,
                            &mut expanded_folders,
                            code,
                            current_file,
                            &mut |msg: &str| log_messages.push(msg.to_string()),
                            &mut rename_dialog,
                            &mut selected_folder,
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
                if is_folder { "folder" } else { "file" },
                new_path.display()
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
                            .id_source(id)
                            .default_open(is_expanded);

                        let state = header.show(ui, |ui| {
                            if is_expanded {
                                Self::render_folder_contents(
                                    ui,
                                    ctx,
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
}
