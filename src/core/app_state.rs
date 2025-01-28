use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use std::fs;
use crate::utils::themes::Theme;
use crate::core::ide::IDE;
use crate::components::code_editor::{Buffer, CursorPosition};
use std::path::Path;
use std::rc::Rc;
use crate::core::file_system::FileSystem;


#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    // File and editor state
    #[serde(with = "path_buf_serde")]
    pub last_project_path: Option<PathBuf>,
    pub open_buffers: Vec<BufferState>,
    pub active_buffer_index: Option<usize>,
    
    // UI state
    pub window_size: (f32, f32),
    pub console_panel_visible: bool,
    pub emulator_panel_visible: bool,
    pub ai_assistant_panel_visible: bool,
    
    // Settings
    pub current_theme: Theme,
    pub ai_api_key: String,
}

// Custom serialization for PathBuf
mod path_buf_serde {
    use serde::{Deserialize, Deserializer, Serializer, Serialize};
    use std::path::PathBuf;

    pub fn serialize<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match path {
            Some(p) => p.to_str().unwrap_or("").serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        Ok(s.map(PathBuf::from))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BufferState {
    pub file_path: String,
    pub cursor_position: CursorPosition,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last_project_path: None,
            open_buffers: Vec::new(),
            active_buffer_index: None,
            window_size: (800.0, 600.0),
            console_panel_visible: false,
            emulator_panel_visible: false,
            ai_assistant_panel_visible: false,
            current_theme: Theme::default(),
            ai_api_key: String::new(),
        }
    }
}

impl AppState {
    pub fn load() -> Self {
        if let Some(config_path) = Self::get_config_path() {
            if let Ok(file_content) = fs::read_to_string(&config_path) {
                if let Ok(state) = serde_json::from_str(&file_content) {
                    return state;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(config_path) = Self::get_config_path() {
            // Ensure the config directory exists
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            let json = serde_json::to_string_pretty(self)?;
            fs::write(config_path, json)?;
        }
        Ok(())
    }

    fn get_config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "zzz", "ide").map(|proj_dirs| {
            proj_dirs.config_dir().join("app_state.json")
        })
    }

    pub fn update_from_ide(&mut self, ide: &IDE) {
        // Existing state updates
        self.last_project_path = ide.file_modal.project_path.clone();
        self.console_panel_visible = ide.show_console_panel;
        self.emulator_panel_visible = ide.show_emulator_panel;
        self.ai_assistant_panel_visible = ide.show_ai_panel;  // Add this line
        self.current_theme = ide.settings_modal.current_theme.clone();
        self.ai_api_key = ide.settings_modal.get_api_key();

        // Save buffer states
        self.open_buffers = ide.code_editor.buffers.iter().map(|buffer| {
            BufferState {
                file_path: buffer.file_path.clone().unwrap_or_default(),
                cursor_position: buffer.cursor_position.clone(),
            }
        }).collect();

        self.active_buffer_index = ide.code_editor.active_buffer_index;
    }

    pub fn apply_to_ide(&self, ide: &mut IDE) {
        // First initialize the file system if we have a project path
        if let Some(project_path) = &self.last_project_path {
            if project_path.exists() {
                let fs = Rc::new(FileSystem::new(project_path.to_str().unwrap()));
                ide.file_modal.file_system = Some(fs);
                ide.file_modal.project_path = Some(project_path.clone());
                
                // Expand the root folder
                ide.file_modal.expanded_folders.insert(project_path.clone());
            }
        }

        // Apply the rest of the saved state
        ide.show_console_panel = self.console_panel_visible;
        ide.show_emulator_panel = self.emulator_panel_visible;
        ide.show_ai_panel = self.ai_assistant_panel_visible;  // Make sure this line is present
        ide.settings_modal.current_theme = self.current_theme.clone();
        ide.settings_modal.set_api_key(self.ai_api_key.clone());

        // Also update the AI Assistant's API key
        ide.ai_assistant.update_api_key(self.ai_api_key.clone());  // Add this line

        // Restore buffers
        for buffer_state in &self.open_buffers {
            let path = Path::new(&buffer_state.file_path);
            if path.exists() {
                if let Some(fs) = &ide.file_modal.file_system {
                    if let Ok(content) = fs.open_file(path) {
                        let mut buffer = Buffer::new();
                        buffer.file_path = Some(buffer_state.file_path.clone());
                        buffer.content = content;
                        buffer.cursor_position = buffer_state.cursor_position.clone();
                        ide.code_editor.buffers.push(buffer);
                    }
                }
            }
        }

        // Ensure no active buffer if there are no buffers
        if ide.code_editor.buffers.is_empty() {
            ide.code_editor.active_buffer_index = None;
        } else {
            ide.code_editor.active_buffer_index = self.active_buffer_index;
        }
    }
}