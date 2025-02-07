use eframe::egui;
use crate::core::git_manager::{GitCommit, GitManager};
use super::code_editor::CodeEditor;
use super::console_panel::ConsolePanel;
use super::file_modal::FileModal;

pub struct GitModal {
    pub show: bool,
    git_manager: Option<GitManager>,
    commits: Vec<GitCommit>,
}

impl GitModal {
    pub fn new() -> Self {
        Self {
            show: false,
            git_manager: None,
            commits: Vec::new(),
        }
    }

    pub fn update_git_manager(&mut self, project_path: Option<std::path::PathBuf>) {
        self.commits.clear();
        self.git_manager = None;
        if let Some(path) = project_path {
            let git_manager = GitManager::new(path.clone());

            if !git_manager.is_git_repo() {
                return;
            }
            match git_manager.get_commits() {
                Ok(commits) => {
                    self.commits = commits;
                    self.git_manager = Some(git_manager);
                },
                Err(_) => {
                    self.commits.clear();
                }
            }
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        file_modal: &mut FileModal,
        code_editor: &mut CodeEditor,
        console_panel: &mut ConsolePanel
    ) {
        if !self.show {
            return;
        }
        let modal_size = egui::vec2(500.0, 500.0);
        egui::Window::new("Git History")
            .fixed_size(modal_size)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_size(modal_size);
                ui.heading("Git History");
                ui.add_space(10.0);
                if let Some(git_manager) = &self.git_manager {
                    if let Ok(commits) = git_manager.get_commits() {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for commit in commits {
                                ui.group(|ui| {
                                    ui.label(format!("Message: {}", commit.message));
                                    ui.label(format!("Author: {}", commit.author));
                                    ui.label(format!("Date: {}", commit.date.format("%Y-%m-%d %H:%M:%S")));
                                    if ui.button("Reset to This Commit").clicked() {
                                        match git_manager.reset_to_commit(&commit.hash) {
                                            Ok(()) => {
                                                file_modal.reload_file_system();
                                                code_editor.reload_all_buffers(
                                                    &file_modal.file_system.as_ref().unwrap(),
                                                    &mut |msg| console_panel.log(msg)
                                                );
                                                console_panel.log(
                                                    &format!("Successfully reset to commit {}", commit.hash)
                                                );
                                            },
                                            Err(e) => console_panel.log(&e),
                                        }
                                    }
                                });
                                ui.add_space(10.0);
                            }
                        });
                    }
                } else {
                    ui.label("No Git repository found in the current project.");
                }
            });
    }
}