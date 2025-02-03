use std::path::PathBuf;
use std::sync::mpsc::{Sender, Receiver, channel};
use crate::components::console_panel::ConsolePanel;
use crate::core::app_state::AppState;

#[derive(Debug, Clone)]
pub enum ExtensionEvent {
    FileSaved(String),
    FileOpened(String),
    ProjectLoaded(PathBuf),
    EditorContentChanged(String),
    BeforeBuild,
    AfterBuild,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum ExtensionCommand {
    Log(String),
    ShowNotification(String),
    OpenFile(PathBuf),
    SetEditorContent(String),
    ExecuteTerminalCommand(String),
}

pub trait ZzzExtension: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_load(&self, context: ExtensionContext);
    fn on_event(&self, event: &ExtensionEvent);
}

#[derive(Clone)]
pub struct ExtensionContext {
    command_sender: Sender<ExtensionCommand>,
    app_state: AppState,
}

impl ExtensionContext {
    pub fn log(&self, message: &str) {
        let _ = self.command_sender.send(ExtensionCommand::Log(message.to_string()));
    }

    pub fn get_app_state(&self) -> &AppState {
        &self.app_state
    }

    pub fn execute_command(&self, command: ExtensionCommand) {
        let _ = self.command_sender.send(command);
    }
}

pub struct ExtensionManager {
    extensions: Vec<Box<dyn ZzzExtension>>,
    command_sender: Sender<ExtensionCommand>,
    command_receiver: Receiver<ExtensionCommand>,
    app_state: AppState,
}

impl ExtensionManager {
    pub fn new(app_state: AppState) -> Self {
        let (command_sender, command_receiver) = channel();
        Self {
            extensions: Vec::new(),
            command_sender,
            command_receiver,
            app_state,
        }
    }

    pub fn load_extension(&mut self, extension: Box<dyn ZzzExtension>) {
        let context = ExtensionContext {
            command_sender: self.command_sender.clone(),
            app_state: self.app_state.clone(),
        };
        extension.on_load(context);
        self.extensions.push(extension);
    }

    pub fn emit_event(&self, event: ExtensionEvent) {
        for extension in &self.extensions {
            extension.on_event(&event);
        }
    }

    pub fn process_commands(&self, console: &mut ConsolePanel) {
        while let Ok(command) = self.command_receiver.try_recv() {
            match command {
                ExtensionCommand::Log(message) => console.log(&message),
                ExtensionCommand::ShowNotification(message) => {
                    // Implement notification system
                    console.log(&format!("Notification: {}", message));
                }
                ExtensionCommand::OpenFile(path) => {
                    // Add logic to open files in editor
                    console.log(&format!("Requested to open: {}", path.display()));
                }
                ExtensionCommand::SetEditorContent(content) => {
                    // Add logic to update editor content
                    console.log("Received editor content update request");
                }
                ExtensionCommand::ExecuteTerminalCommand(cmd) => {
                    // Add logic to execute terminal commands
                    console.log(&format!("Requested to execute: {}", cmd));
                }
            }
        }
    }
}