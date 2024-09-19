use eframe::egui;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use std::collections::HashSet;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use crate::core::terminal::Terminal;
use crate::core::file_system::FileSystem;
use crate::core::lsp_client::LspClient;
use lsp_types::{
    InitializeParams, 
    ClientCapabilities, 
    Url, 
    DidChangeTextDocumentParams, 
    VersionedTextDocumentIdentifier, 
    TextDocumentContentChangeEvent
};
use tokio::runtime::Runtime;

pub struct FileModal {
    pub show: bool,
    pub file_system: Option<Rc<FileSystem>>,
    pub project_path: Option<PathBuf>,
    pub expanded_folders: HashSet<PathBuf>,
    pub selected_folder: Option<PathBuf>,
    pub selected_item: Option<PathBuf>,
    editing_item: Option<(PathBuf, String)>,
    creating_item: Option<(PathBuf, String, bool)>,
    context_menu: Option<ContextMenuState>,
    new_item_focus: bool,
    lsp_client: Option<Arc<LspClient>>,
    runtime: Arc<Runtime>,
    terminal: Arc<Mutex<Terminal>>,
}

struct ContextMenuState {
    path: PathBuf,
    is_dir: bool,
    pos: egui::Pos2,
}

impl FileModal {
    pub fn new(runtime: Arc<Runtime>, terminal: Arc<Mutex<Terminal>>) -> Self {
        Self {
            show: false,
            file_system: None,
            project_path: None,
            expanded_folders: HashSet::new(),
            selected_folder: None,
            selected_item: None,
            editing_item: None,
            creating_item: None,
            context_menu: None,
            new_item_focus: false,
            lsp_client: None,
            runtime,
            terminal,
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
                            self.open_folder(log);
                        }
                        
                        let project_path = self.project_path.clone();
                        if project_path.is_some() {
                            if ui.button("New File").clicked() {
                                if let Some(path) = &project_path {
                                    self.start_create_item(false, path);
                                }
                            }
                            if ui.button("New Folder").clicked() {
                                if let Some(path) = &project_path {
                                    self.start_create_item(true, path);
                                }
                            }
                            if ui.button("Save").clicked() {
                                self.save_current_file(code, current_file, log);
                            }
                        }
                    });
                    
                    ui.separator();

                    if let (Some(fs), Some(project_path)) = (&self.file_system, &self.project_path) {
                        let fs = fs.clone();
                        let project_path = project_path.clone();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            self.render_folder_contents(
                                ui, ctx, &project_path, &fs, code,
                                current_file, log, 0,
                            );
                        });
                    } else {
                        ui.label("No project opened. Click 'Open Folder' to start.");
                    }
                });
            });

        self.handle_context_menu(ctx, log);
    }
    fn render_folder_contents(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        folder: &Path,
        fs: &Rc<FileSystem>,
        code: &mut String,
        current_file: &mut Option<String>,
        log: &mut dyn FnMut(&str),
        indent_level: usize,
    ) {       
        if let Ok(entries) = fs.list_directory(folder) {
            for entry in entries {
                let path = folder.join(&entry.name);
                let is_dir = entry.is_dir;
                let is_expanded = self.expanded_folders.contains(&path);

                ui.horizontal(|ui| {
                    ui.add_space(indent_level as f32 * 20.0);

                    let is_editing = self.editing_item.as_ref().map_or(false, |(edit_path, _)| edit_path == &path);
                    let is_selected = self.selected_item.as_ref() == Some(&path);

                    if is_editing {
                        if let Some((_, ref mut name)) = self.editing_item {
                            let response = ui.text_edit_singleline(name);
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                self.finish_rename(log);
                            }
                        }
                    } else {
                        let text = if is_dir {
                            format!("{}", entry.name)
                        } else {
                            format!(" {}", entry.name)
                        };

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
                            self.selected_item = Some(path.clone());
                            if is_dir {
                                if is_expanded {
                                    self.expanded_folders.remove(&path);
                                } else {
                                    self.expanded_folders.insert(path.clone());
                                }
                                self.selected_folder = Some(path.clone());
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

                        if response.double_clicked() {
                            self.start_rename(&path);
                        }

                        if response.secondary_clicked() {
                            if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                                let screen_pos = pointer_pos;
                                self.context_menu = Some(ContextMenuState {
                                    path: path.clone(),
                                    is_dir,
                                    pos: screen_pos,
                                });
                            }
                        }

                        if response.hovered() {
                            let hover_text = if is_dir {
                                "Click to expand/collapse, double-click to rename"
                            } else {
                                "Click to open, double-click to rename"
                            };
                            response.on_hover_text(hover_text);
                        }
                    }
                });

                if is_dir && (is_expanded || self.creating_item.as_ref().map_or(false, |(parent, _, _)| parent == &path)) {
                    self.render_folder_contents(
                        ui, ctx, &path, fs, code,
                        current_file, log, indent_level + 1,
                    );
                }
            }
        } else {
            log(&format!("Error reading directory: {}", folder.display()));
        }

        // Render item being created
        let mut item_created = false;
        if let Some((parent, name, _is_folder)) = &mut self.creating_item {
            if parent == folder {
                ui.horizontal(|ui| {
                    ui.add_space((indent_level + 1) as f32 * 20.0);
                    let response = ui.text_edit_singleline(name);
                    if self.new_item_focus {
                        response.request_focus();
                        self.new_item_focus = false;
                    }
                    if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        item_created = true;
                    }
                });
            }
        }
        if item_created {
            if let Some((parent, name, is_folder)) = self.creating_item.take() {
                self.finish_create_item(&parent, &name, is_folder, log);
            }
        }
    }
    
    
    fn handle_context_menu(&mut self, ctx: &egui::Context, log: &mut dyn FnMut(&str)) {
        if let Some(menu_state) = &self.context_menu {
            let pos = menu_state.pos;
            let path = menu_state.path.clone();
            let is_dir = menu_state.is_dir;

            egui::Area::new(egui::Id::new("context_menu"))
                .fixed_pos(pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_max_width(150.0);
                            
                            if ui.button("Rename").clicked() {
                                self.start_rename(&path);
                                self.context_menu = None;
                            }
                            if ui.button("Delete").clicked() {
                                self.delete_item(&path, log);
                                self.context_menu = None;
                            }
                            if is_dir {
                                if ui.button("New File").clicked() {
                                    self.start_create_item(false, &path);
                                    self.context_menu = None;
                                }
                                if ui.button("New Folder").clicked() {
                                    self.start_create_item(true, &path);
                                    self.context_menu = None;
                                }
                            }
                        });
                });

            // Close the context menu if clicked outside
            if ctx.input(|i| i.pointer.any_click()) {
                if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                    let menu_rect = egui::Rect::from_min_size(pos, egui::Vec2::new(150.0, 100.0)); // Approximate size
                    if !menu_rect.contains(pointer_pos) {
                        self.context_menu = None;
                    }
                }
            }
        }
    }

    fn start_rename(&mut self, path: &Path) {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        self.editing_item = Some((path.to_path_buf(), name));
    }

    fn finish_rename(&mut self, log: &mut dyn FnMut(&str)) {
        if let Some((old_path, new_name)) = self.editing_item.take() {
            let new_path = old_path.with_file_name(&new_name);
            if let Some(fs) = &self.file_system {
                match fs.rename_file(&old_path, &new_path) {
                    Ok(_) => log(&format!("Renamed '{}' to '{}'", old_path.display(), new_path.display())),
                    Err(e) => log(&format!("Error renaming: {}", e)),
                }
            }
        }
    }

    fn start_create_item(&mut self, is_folder: bool, parent: &Path) {
        let name = if is_folder { "New Folder".to_string() } else { "New File.txt".to_string() };
        self.creating_item = Some((parent.to_path_buf(), name, is_folder));
        self.expanded_folders.insert(parent.to_path_buf());
        self.new_item_focus = true;
    }   

    fn finish_create_item(&mut self, parent: &Path, name: &str, is_folder: bool, log: &mut dyn FnMut(&str)) {
        if let Some(fs) = &self.file_system {
            let new_path = parent.join(name);
            if is_folder {
                match fs.create_directory(&new_path) {
                    Ok(_) => log(&format!("Created new folder: {}", new_path.display())),
                    Err(e) => log(&format!("Error creating folder: {}", e)),
                }
            } else {
                match fs.create_new_file(parent, name) {
                    Ok(_) => log(&format!("Created new file: {}", new_path.display())),
                    Err(e) => log(&format!("Error creating file: {}", e)),
                }
            }
            self.expanded_folders.insert(parent.to_path_buf());
        }
    }

    fn delete_item(&mut self, path: &Path, log: &mut dyn FnMut(&str)) {
        if let Some(fs) = &self.file_system {
            match fs.delete_file(path) {
                Ok(_) => {
                    self.selected_item = None;
                    if path.is_dir() {
                        self.expanded_folders.remove(path);
                    }
                    log(&format!("Deleted: {}", path.display()));
                },
                Err(e) => log(&format!("Error deleting {}: {:?}", path.display(), e)),
            }
        }
    }

    
    pub fn open_folder(&mut self, log: &mut dyn FnMut(&str)) {
        if let Some(folder_path) = FileDialog::new().pick_folder() {
            self.project_path = Some(folder_path.clone());
            let fs = Rc::new(FileSystem::new(folder_path.to_str().unwrap()));
            self.file_system = Some(fs.clone());
            self.expanded_folders.clear();
            self.expanded_folders.insert(folder_path.clone());
            log(&format!("Opened project: {}", folder_path.display()));

            // Set working directory for the terminal
            if let Ok(terminal) = self.terminal.lock() {
                terminal.set_working_directory(folder_path.clone());
            }

            // Initialize LSP client
            self.runtime.block_on(async {
                let lsp_client = Arc::new(LspClient::new());

                let root_uri = Url::from_file_path(&folder_path).expect("Failed to create URL from path");
                let init_params = InitializeParams {
                    root_uri: Some(root_uri),
                    capabilities: ClientCapabilities::default(),
                    ..InitializeParams::default()
                };
                match lsp_client.initialize(init_params).await {
                    Ok(_) => {
                        self.lsp_client = Some(lsp_client);
                        log("LSP client initialized successfully");
                    },
                    Err(e) => {
                        log(&format!("Failed to initialize LSP client: {:?}", e));
                    }
                }
            });

        }
    }

    pub fn notify_file_change(&self, file_path: &str, content: &str) {
        if let Some(lsp_client) = &self.lsp_client {
            let uri = Url::from_file_path(file_path).expect("Failed to create URL from path");
            let lsp_client_clone = Arc::clone(lsp_client);
            let content = content.to_string();
            self.runtime.spawn(async move {
                lsp_client_clone.did_change(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 0, // You might want to implement proper versioning
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: content,
                    }],
                }).await;
            });
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
}

