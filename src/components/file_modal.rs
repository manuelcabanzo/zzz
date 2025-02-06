use eframe::egui;
use std::path::{PathBuf, Path};
use std::collections::HashSet;
use std::rc::Rc;
use rfd::FileDialog;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::core::file_system::FileSystem;
use crate::components::code_editor::CodeEditor;

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
    is_initializing: AtomicBool,
}

struct ContextMenuState {
    path: PathBuf,
    is_dir: bool,
    pos: egui::Pos2,
}

impl FileModal {
    pub fn new() -> Self {
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
            is_initializing: AtomicBool::new(false),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, code_editor: &mut CodeEditor, log: &mut dyn FnMut(&str)) {
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
                        
                        if self.project_path.is_some() {
                            if ui.button("New File").clicked() {
                                let target_path = self.selected_folder.as_ref()
                                    .or(self.project_path.as_ref())
                                    .map(|p| p.clone())
                                    .unwrap();
                                self.start_create_item(false, &target_path);
                            }
                            if ui.button("New Folder").clicked() {
                                let target_path = self.selected_folder.as_ref()
                                    .or(self.project_path.as_ref())
                                    .map(|p| p.clone())
                                    .unwrap();
                                self.start_create_item(true, &target_path);
                            }
                            if ui.button("Save").clicked() {
                                self.save_current_file(code_editor, log);
                            }
                            if ui.button("Collapse All").clicked() {
                                self.collapse_all_folders();
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
                                ui, ctx, &project_path, &fs, code_editor,
                                log, 0,
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
        code_editor: &mut CodeEditor,
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
                            
                            // Request focus if it's the first time
                            if self.new_item_focus {
                                response.request_focus();
                                self.new_item_focus = false; // After focusing, reset the flag
                            }
    
                            // Check for pressing Enter (to finish rename) or Esc (to cancel rename)
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                self.finish_rename(log);
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.cancel_rename();
                            }
    
                            // Check for clicking outside the input field
                            if response.clicked() {
                                // Do nothing, it was clicked inside the input box
                            } else if response.lost_focus() {
                                // Clicked outside the input box, cancel rename
                                self.cancel_rename();
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
                                self.selected_folder = Some(path.parent().unwrap().to_path_buf());
                                match fs.open_file(&path) {
                                    Ok(content) => {
                                        code_editor.open_file(content, path.to_str().unwrap().to_string());
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
                        ui,
                        ctx,
                        &path,
                        fs,
                        code_editor,
                        log,
                        indent_level + 1
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
    
    fn cancel_rename(&mut self) {
        self.editing_item = None;
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
        self.new_item_focus = true; // Mark that we need to focus on the input field
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
        if self.is_initializing.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            if let Some(folder_path) = FileDialog::new().pick_folder() {
                if self.project_path.as_ref() == Some(&folder_path) {
                    log("Project already open");
                    self.is_initializing.store(false, Ordering::SeqCst);
                    return;
                }
                
                // Clear existing state first
                self.expanded_folders.clear();
                self.selected_folder = None;
                self.selected_item = None;
                self.editing_item = None;
                self.creating_item = None;
                self.context_menu = None;
                
                // Set up new project
                self.project_path = Some(folder_path.clone());
                let fs = Rc::new(FileSystem::new(folder_path.to_str().unwrap()));
                self.file_system = Some(fs);
                
                // Expand root folder
                self.expanded_folders.insert(folder_path.clone());
                log(&format!("Opened project: {}", folder_path.display()));
            }
            self.is_initializing.store(false, Ordering::SeqCst);
        } else {
            log("Folder opening already in progress");
        }
    }

    fn collapse_all_folders(&mut self) {
        self.expanded_folders.clear();
    }

    pub fn reload_all_buffers(&mut self, code_editor: &mut CodeEditor, log: &mut dyn FnMut(&str)) {
        let buffers = code_editor.buffers.drain(..).collect::<Vec<_>>();
        
        for mut buffer in buffers {
            if let Some(file_path) = &buffer.file_path {
                match self.file_system.as_ref().unwrap().open_file(Path::new(file_path)) {
                    Ok(content) => {
                        buffer.content = content;
                        buffer.is_modified = false;
                        code_editor.buffers.push(buffer);
                    },
                    Err(e) => log(&format!("Failed to reload {}: {}", file_path, e))
                }
            } else {
                code_editor.buffers.push(buffer);
            }
        }
    }
    
    pub fn save_current_file(&self, code_editor: &mut CodeEditor, log: &mut dyn FnMut(&str)) {
        if let Some(buffer) = code_editor.get_active_buffer() {
            if let Some(file_path) = &buffer.file_path {
                if let Some(fs) = &self.file_system {
                    let path = Path::new(file_path);
                    match fs.save_file(path, &buffer.content) {
                        Ok(_) => {
                            log(&format!("Saved file: {}", file_path));
                            if let Some(buffer) = code_editor.get_active_buffer_mut() {
                                buffer.is_modified = false;
                            }
                        },
                        Err(e) => log(&format!("Error saving file {}: {}", file_path, e)),
                    }
                }
            }
        } else {
            log("No file is currently open.");
        }
    }

    pub fn search_files(&self, query: &str) -> Vec<String> {
        let mut results = Vec::new();
        if let Some(fs) = &self.file_system {
            if let Some(project_path) = &self.project_path {
                let excluded_dirs = vec![
                    "build", "target", "out", "bin", "node_modules", ".gradle", "gradle", "captures",
                    ".git", ".svn", ".idea", ".vscode", "app/build", "androidTest", "test", "debug",
                    "release", "shared/build", "commonMain", "androidMain", "iosMain", "__MACOSX",
                    ".DS_Store", "*.xcodeproj", "*.iml",
                ];
                self.search_directory(fs, project_path, query, &mut results, &excluded_dirs);
            }
        }
        results
    }

    fn search_directory(&self, fs: &Rc<FileSystem>, dir: &Path, query: &str, results: &mut Vec<String>, excluded_dirs: &[&str]) {
        let query_lower = query.to_lowercase();
        if let Ok(entries) = fs.list_directory(dir) {
            for entry in entries {
                let path = dir.join(&entry.name);
                if entry.is_dir {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if excluded_dirs.iter().any(|&excluded| dir_name == excluded || dir_name.starts_with(excluded) || dir_name.contains(excluded)) {
                        continue;
                    }
                    self.search_directory(fs, &path, query, results, excluded_dirs);
                } else {
                    if entry.name.to_lowercase().contains(&query_lower) {
                        results.push(path.to_str().unwrap().to_string());
                    }
                }
            }
        }
    }

    pub fn get_all_file_paths(&self) -> Vec<String> {
        let mut all_paths = Vec::new();
        
        if let (Some(fs), Some(project_path)) = (&self.file_system, &self.project_path) {
            // Reuse excluded directories from search_files to maintain consistency
            let excluded_dirs = vec![
                "build", "target", "out", "bin", "node_modules", ".gradle", "gradle", "captures",
                ".git", ".svn", ".idea", ".vscode", "app/build", "androidTest", "test", "debug",
                "release", "shared/build", "commonMain", "androidMain", "iosMain", "__MACOSX",
                ".DS_Store", "*.xcodeproj", "*.iml",
            ];
            
            // Use a recursive closure to traverse the directory tree
            fn collect_files(
                fs: &Rc<FileSystem>,
                dir: &Path,
                paths: &mut Vec<String>,
                excluded_dirs: &[&str]
            ) {
                if let Ok(entries) = fs.list_directory(dir) {
                    for entry in entries {
                        let path = dir.join(&entry.name);
                        
                        if entry.is_dir {
                            let dir_name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                                
                            // Skip excluded directories
                            if excluded_dirs.iter().any(|&excluded| 
                                dir_name == excluded || 
                                dir_name.starts_with(excluded) || 
                                dir_name.contains(excluded)
                            ) {
                                continue;
                            }
                            
                            collect_files(fs, &path, paths, excluded_dirs);
                        } else {
                            // Only add files that are likely to be source code or text
                            let extension = path.extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or("");
                                
                            let is_text_file = matches!(extension, 
                                "rs" | "ts" | "js" | "py" | "java" | "kt" | "cpp" | "h" | "hpp" |
                                "c" | "cs" | "go" | "rb" | "php" | "html" | "css" | "json" | "yaml" |
                                "yml" | "toml" | "md" | "txt" | "xml" | "gradle" | "properties" |
                                "sh" | "bat" | "cmd" | "ps1" | "sql" | "swift" | "m" | "mm"
                            );
                            
                            if is_text_file {
                                if let Some(path_str) = path.to_str() {
                                    paths.push(path_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
            
            collect_files(fs, project_path, &mut all_paths, &excluded_dirs);
        }
        
        all_paths
    }
    
    pub fn reload_file_system(&mut self) {
        if let Some(project_path) = &self.project_path {
            self.file_system = Some(Rc::new(FileSystem::new(
                project_path.to_str().unwrap()
            )));
            // Clear cached folder states
            self.expanded_folders.clear();
            self.expanded_folders.insert(project_path.clone());
        }
    }
    
    pub fn open_file(&mut self, file_path: &str, code_editor: &mut CodeEditor) {
        if let Some(fs) = &self.file_system {
            let path = Path::new(file_path);
            match fs.open_file(path) {
                Ok(content) => {
                    code_editor.open_file(content, file_path.to_string());
                }
                Err(e) => eprintln!("Error opening file {}: {}", file_path, e),
            }
        }
    }
}