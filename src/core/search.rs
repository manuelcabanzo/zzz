use crate::core::file_system::FileSystem;
use std::rc::Rc;
use std::path::Path;
use egui::{Context, TextEdit, ScrollArea};
use super::ide::IDE;

#[derive(Clone)]
pub struct SearchResult {
    pub line_number: usize,
    pub line_content: String,
    pub file_path: Option<String>,
}

pub fn perform_current_file_search(ide: &mut IDE) {
    if let Some(buffer) = ide.code_editor.get_active_buffer() {
        let content = &buffer.content;
        ide.search_results = content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(&ide.search_query))
            .map(|(line_num, line)| SearchResult {
                line_number: line_num + 1,
                line_content: line.to_string(),
                file_path: buffer.file_path.clone(),
            })
            .collect();
    }
}

pub fn perform_project_search(ide: &mut IDE) {
    const MAX_RESULTS: usize = 100;
    let mut results = Vec::new();

    let excluded_dirs = vec![
        "build", "target", "out", "bin", "node_modules", ".gradle", "gradle", "captures",
        ".git", ".svn", ".idea", ".vscode", "app/build", "androidTest", "test", "debug",
        "release", "shared/build", "commonMain", "androidMain", "iosMain", "__MACOSX",
        ".DS_Store", "*.xcodeproj", "*.iml",
    ];

    if let Some(fs) = &ide.file_modal.file_system {
        if let Some(project_path) = &ide.file_modal.project_path {
            let query = ide.search_query.trim();
            if query.len() >= 2 {
                search_in_directory_with_exclusions(
                    fs, project_path, query, &mut results, MAX_RESULTS, &excluded_dirs,
                );
            }
        }
    }
    ide.search_results = results;
}

fn search_in_directory_with_exclusions(
    fs: &Rc<FileSystem>,
    dir: &Path,
    query: &str,
    results: &mut Vec<SearchResult>,
    max_results: usize,
    excluded_dirs: &[&str],
) {
    if results.len() >= max_results {
        return;
    }

    if let Ok(entries) = fs.list_directory(dir) {
        for entry in entries {
            if results.len() >= max_results {
                break;
            }

            let path = dir.join(&entry.name);

            if entry.is_dir {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if excluded_dirs.iter().any(|&excluded| dir_name == excluded || dir_name.starts_with(excluded) || dir_name.contains(excluded)) {
                    continue;
                }

                search_in_directory_with_exclusions(fs, &path, query, results, max_results, excluded_dirs);
            } else {
                let file_ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
                let skippable_extensions = ["png", "jpg", "jpeg", "gif", "svg", "pdf", "zip", "tar", "gz", "class", "jar", "so", "dll", "dylib", "o", "a"];

                if skippable_extensions.contains(&file_ext) {
                    continue;
                }

                if let Ok(content) = fs.open_file(&path) {
                    let file_results = content
                        .lines()
                        .enumerate()
                        .filter(|(_, line)| line.contains(query))
                        .take(10)
                        .map(|(line_num, line)| SearchResult {
                            line_number: line_num + 1,
                            line_content: line.to_string(),
                            file_path: Some(path.to_str().unwrap().to_string()),
                        })
                        .collect::<Vec<_>>();

                    results.extend(file_results);
                }
            }
        }
    }
}

pub fn show_search_modal(ide: &mut IDE, ctx: &Context) {
    let is_project_search = ide.show_project_search_modal;
    let modal_title = if is_project_search { "Project Search" } else { "Current File Search" };

    if ide.show_current_file_search_modal || ide.show_project_search_modal {
        egui::Window::new(modal_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let text_edit = TextEdit::singleline(&mut ide.search_query).hint_text("Search...");

                    let response = ui.add(text_edit);
                    if ide.search_focus_requested {
                        response.request_focus();
                        ide.search_focus_requested = false;
                    }

                    if !ide.search_query.is_empty() {
                        if is_project_search {
                            perform_project_search(ide);
                        } else {
                            perform_current_file_search(ide);
                        }
                    }

                    ScrollArea::vertical().show(ui, |ui| {
                        for result in ide.search_results.iter() {
                            let display_text = if is_project_search {
                                format!(
                                    "{}:{} - {}",
                                    result.file_path.as_ref().unwrap_or(&"Unknown".to_string()),
                                    result.line_number,
                                    result.line_content.trim()
                                )
                            } else {
                                format!("Line {}: {}", result.line_number, result.line_content.trim())
                            };

                            let response = ui.button(display_text);
                            if response.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // First, clear any existing highlights
                                ide.code_editor.search_highlight_text = None;
                                ide.code_editor.search_selected_line = None;
                                ide.code_editor.selected_match_position = None;
                                
                                // Open the file if it's a project search
                                if is_project_search {
                                    if let Some(file_path) = &result.file_path {
                                        ide.file_modal.open_file(file_path, &mut ide.code_editor);
                                    }
                                }
                            
                                // Calculate absolute position of the match in the file
                                if let Some(buffer) = ide.code_editor.get_active_buffer() {
                                    let content = &buffer.content;
                                    let mut line_start = 0;
                                    for _ in 0..result.line_number.saturating_sub(1) {
                                        if let Some(next_line) = content[line_start..].find('\n') {
                                            line_start += next_line + 1;
                                        }
                                    }
                                    
                                    if let Some(column_offset) = result.line_content.find(&ide.search_query) {
                                        let match_start = line_start + column_offset;
                                        let match_end = match_start + ide.search_query.len();
                                        ide.code_editor.selected_match_position = Some((match_start, match_end));
                                        
                                        // Set cursor position
                                        if let Some(buffer) = ide.code_editor.get_active_buffer_mut() {
                                            buffer.set_cursor_position(result.line_number, column_offset);
                                            buffer.is_modified = false;
                                        }
                                    }
                                }
                            
                                // Set up highlighting
                                ide.code_editor.search_selected_line = Some(result.line_number);
                                ide.code_editor.search_highlight_text = Some(ide.search_query.clone());
                                ide.code_editor.search_highlight_expires_at = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs_f64(0.5)
                                );
                            
                                // Request a repaint to ensure highlighting is visible
                                ctx.request_repaint();
                            
                                // Close the search modals
                                ide.show_current_file_search_modal = false;
                                ide.show_project_search_modal = false;
                            }
                        }
                    });
                });
            });
    }
}