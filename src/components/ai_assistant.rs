use eframe::egui;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::{VecDeque, HashSet};
use chrono::Local;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    content: String,
    is_user: bool,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContextFile {
    path: String,
    content: String,
    is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TogetherAIRequest {
    model: String,
    prompt: String,
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    top_k: u32,
    repetition_penalty: f32,
    stop: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

pub struct AIAssistant {
    api_key: String,
    input_text: String,
    chat_history: VecDeque<Message>,
    context_files: Vec<ContextFile>,
    is_loading: bool,
    http_client: Client,
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
    scroll_to_bottom: bool,
    context_window: usize,
    debug_messages: VecDeque<String>,
    panel_height: f32,
    runtime: Arc<Runtime>,
    last_ai_response: Option<String>,
    model: String,
    show_file_selector: bool,
    available_files: Vec<String>,
    selected_files: HashSet<String>,
}


impl AIAssistant {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_MS: u64 = 1000;
    const MAX_CHAT_HISTORY: usize = 100;
    const MAX_DEBUG_MESSAGES: usize = 10;

    pub fn new(api_key: String, runtime: Arc<Runtime>) -> Self {
        let (tx, rx) = mpsc::channel(32);
        
        Self {
            api_key,
            input_text: String::new(),
            chat_history: VecDeque::with_capacity(Self::MAX_CHAT_HISTORY),
            context_files: Vec::new(),
            is_loading: false,
            http_client: Client::new(),
            tx,
            rx,
            scroll_to_bottom: false,
            context_window: 5,
            debug_messages: VecDeque::with_capacity(Self::MAX_DEBUG_MESSAGES),
            panel_height: 600.0,
            runtime,
            last_ai_response: None,
            model: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(),
            show_file_selector: false,
            available_files: Vec::new(),
            selected_files: HashSet::new(),
        }
    }

    pub fn update_model(&mut self, new_model: String) {
        self.model = new_model;
    }

    pub fn update_available_files(&mut self, file_paths: Vec<String>) {
        self.available_files = file_paths;
    }

    fn format_chat_messages(&self, file_content: &str, current_question: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();
        
        // Create a comprehensive system message with all active context files and current file
        let mut context_content = self.context_files
            .iter()
            .filter(|f| f.is_active)
            .map(|f| format!("File: {}\n```\n{}\n```", f.path, f.content))
            .collect::<Vec<_>>();
        
        // Add current file content if provided
        if !file_content.is_empty() {
            context_content.push(format!("Current File:\n```\n{}\n```", file_content));
        }
    
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: format!(
                "You are an AI programming assistant in an IDE. You have access to the following files:\n\n{}",
                context_content.join("\n\n")
            ),
        });
    
        // Add conversation history
        for msg in self.chat_history.iter().take(self.context_window) {
            messages.push(ChatMessage {
                role: if msg.is_user { "user" } else { "assistant" }.to_string(),
                content: msg.content.clone(),
            });
        }
    
        // Add the current question
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: current_question.to_string(),
        });
    
        messages
    }

    async fn make_api_request(
        client: &Client,
        api_key: &str,
        request: &ChatRequest,
    ) -> Result<String, String> {
        let mut retries = 0;
        
        while retries < Self::MAX_RETRIES {
            println!("Attempt {} of {}", retries + 1, Self::MAX_RETRIES);
            
            let result = client
                .post("https://api.together.xyz/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(request)
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    println!("API Response Status: {}", status);
                    
                    if status.is_success() {
                        match response.text().await {
                            Ok(text) => {
                                println!("Raw API Response: {}", text);
                                return Ok(text);
                            }
                            Err(e) => {
                                return Err(format!("Failed to read response: {}", e));
                            }
                        }
                    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        println!("Rate limit exceeded, waiting before retry...");
                    } else if status == reqwest::StatusCode::INTERNAL_SERVER_ERROR {
                        println!("Internal server error, attempting retry...");
                    } else {
                        return Err(format!("API returned error status: {}", status));
                    }
                }
                Err(e) => {
                    if e.is_timeout() {
                        println!("Request timed out, attempting retry...");
                    } else if e.is_connect() {
                        println!("Connection error, attempting retry...");
                    } else {
                        return Err(format!("Network error: {}", e));
                    }
                }
            }

            retries += 1;
            if retries < Self::MAX_RETRIES {
                tokio::time::sleep(std::time::Duration::from_millis(Self::RETRY_DELAY_MS)).await;
            }
        }

        Err("Max retries exceeded".to_string())
    }

    pub fn is_api_key_valid(&self) -> bool {
        self.api_key.len() >= 32 && !self.api_key.chars().all(|c| c.is_whitespace())
    }

    fn add_debug_message(&mut self, msg: String) {
        println!("AI Assistant Debug: {}", msg);
        self.debug_messages.push_back(msg);
        while self.debug_messages.len() > Self::MAX_DEBUG_MESSAGES {
            self.debug_messages.pop_front();
        }
    }

    fn add_message(&mut self, content: String, is_user: bool) {
        let timestamp = Local::now().format("%H:%M").to_string();
        println!("Adding message - Content: {}, User: {}", content, is_user);
        
        self.chat_history.push_back(Message {
            content,
            is_user,
            timestamp,
        });
        
        while self.chat_history.len() > Self::MAX_CHAT_HISTORY {
            self.chat_history.pop_front();
        }
        
        self.scroll_to_bottom = true;
    }

    pub fn update_api_key(&mut self, new_key: String) {
        self.api_key = new_key;
    }

    fn render_chat_history(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .max_height(self.panel_height - 200.0)
            .min_scrolled_height(200.0)
            .auto_shrink([false; 2])
            .stick_to_bottom(self.scroll_to_bottom)
            .show(ui, |ui| {
                ui.add_space(8.0);
                
                if self.chat_history.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.label("No messages yet. Start a conversation!");
                    });
                }
                
                for message in &self.chat_history {
                    let bg_color = if message.is_user {
                        ui.style().visuals.extreme_bg_color
                    } else {
                        ui.style().visuals.code_bg_color
                    };

                    egui::Frame::none()
                        .fill(bg_color)
                        .outer_margin(egui::vec2(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&message.timestamp)
                                        .small()
                                        .color(egui::Color32::GRAY));
                                });
                                
                                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut message.content.as_str())
                                            .desired_width(ui.available_width() - 16.0)
                                            .interactive(false)
                                            .frame(false)
                                    );
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            });
    }

    fn render_input_area(&mut self, ui: &mut egui::Ui, code_editor: &mut super::code_editor::CodeEditor) {
        ui.horizontal(|ui| {
            let text_edit = ui.add_sized(
                [ui.available_width() - 160.0, 80.0],
                egui::TextEdit::multiline(&mut self.input_text)
                    .hint_text("Ask about your code or request changes...")
                    .desired_rows(3)
            );
        
            ui.horizontal(|ui| {
        
                let last_response = self.last_ai_response.clone().unwrap_or_default();
                let has_code_block = extract_code_block(&last_response).trim().len() > 0;

                let send_button = ui.add_sized(
                    [70.0, 40.0],
                    egui::Button::new(
                        egui::RichText::new(if self.is_loading { "⌛" } else { "Send" })
                            .size(16.0)
                    )
                );

                if has_code_block {
                    ui.vertical(|ui| {
                        let apply_button = ui.add_sized(
                            [70.0, 40.0],
                            egui::Button::new(
                                egui::RichText::new("Apply Code")
                                    .size(16.0)
                            )
                        );

                        if apply_button.clicked() {
                            if let Some(active_buffer) = code_editor.get_active_buffer_mut() {
                                let code_block = extract_code_block(&last_response);
                                if !code_block.is_empty() {
                                    active_buffer.content = code_block.trim().to_string();
                                    active_buffer.is_modified = true;
                                    self.last_ai_response = None;
                                }
                            }
                        }
                    });
                }

                if (text_edit.lost_focus() && 
                    ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) || 
                    send_button.clicked()) && 
                    !self.input_text.trim().is_empty() && 
                    !self.is_loading 
                {
                    let question = std::mem::take(&mut self.input_text);
                    self.add_message(question.clone(), true);
                    self.is_loading = true;
                    
                    let file_content = code_editor.get_active_content();
                    let messages = self.format_chat_messages(&file_content, &question);
                    
                    let tx = self.tx.clone();
                    let api_key = self.api_key.clone();
                    let client = self.http_client.clone();
                    let model = self.model.clone();

                    self.runtime.spawn(async move {
                        let request = ChatRequest {
                            model,
                            messages,
                        };
                    
                        println!("Sending request to Together AI: {:?}", request);
                    
                        let api_response = Self::make_api_request(&client, &api_key, &request).await;
                        
                        match api_response {
                            Ok(text) => {
                                match serde_json::from_str::<ChatResponse>(&text) {
                                    Ok(response) => {
                                        if let Some(choice) = response.choices.first() {
                                            let _ = tx.send(choice.message.content.trim().to_string()).await;
                                        } else {
                                            let _ = tx.send("No response generated".to_string()).await;
                                        }
                                    },
                                    Err(e) => {
                                        let _ = tx.send(format!(
                                            "Error parsing response: {}. Raw response: {}", 
                                            e, 
                                            text
                                        )).await;
                                    }
                                }
                            },
                            Err(e) => {
                                let _ = tx.send(format!("Request failed: {}", e)).await;
                            }
                        }
                    });
                }
            });
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, code_editor: &mut super::code_editor::CodeEditor) {
        if !self.is_api_key_valid() {
            ui.colored_label(
                egui::Color32::RED, 
                "Invalid API key format. Please check your Together AI API key in Settings"
            );
            return;
        }

        egui::Frame::none()
            .fill(ui.style().visuals.window_fill())
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.heading("AI Assistant");
                    ui.add_space(8.0);

                    // File Selection UI
                    if ui.button("Manage Context Files").clicked() {
                        self.show_file_selector = !self.show_file_selector;
                    }

                    if self.show_file_selector {
                        self.show_file_selector_ui(ui);
                    }

                    // Active Context Files Display
                    if !self.context_files.is_empty() {
                        ui.collapsing("Active Context Files", |ui| {
                            // Create a vector to store indices of files to remove
                            let mut to_remove = Vec::new();
                            
                            for (index, file) in self.context_files.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut file.is_active, "");
                                    ui.label(&file.path);
                                    if ui.small_button("×").clicked() {
                                        // Mark this index for removal
                                        to_remove.push(index);
                                        // Remove from selected_files as well
                                        self.selected_files.remove(&file.path);
                                        println!("Removing file from context: {}", file.path);
                                    }
                                });
                            }
                            
                            // Remove marked files in reverse order to maintain correct indices
                            for &index in to_remove.iter().rev() {
                                self.context_files.remove(index);
                            }
                        });
                    }

                    ui.add_space(8.0);
                    
                    // Chat History
                    self.render_chat_history(ui);
                    
                    ui.add_space(8.0);
                    
                    // Input Area
                    self.render_input_area(ui, code_editor);
                });
            });

        // Process incoming messages
        while let Ok(response) = self.rx.try_recv() {
            if response.starts_with("Error") || 
               response.starts_with("Network error") || 
               response.starts_with("API error") 
            {
                self.add_debug_message(response.clone());
            }
            
            self.last_ai_response = Some(response.clone());
            self.add_message(response, false);
            self.is_loading = false;
        }
    }
    
    fn show_file_selector_ui(&mut self, ui: &mut egui::Ui) {
        // Add debug prints to see what files are available
        println!("Available files: {:?}", self.available_files);
        println!("Currently selected files: {:?}", self.selected_files);
    
        egui::Window::new("Select Context Files")
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    // Show the count of available files
                    ui.label(format!("Total files available: {}", self.available_files.len()));
                    
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for file_path in &self.available_files {
                                let mut is_selected = self.selected_files.contains(file_path);
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut is_selected, "").changed() {
                                        println!("Checkbox clicked for file: {}", file_path);
                                        if is_selected {
                                            println!("Adding file to selected files: {}", file_path);
                                            self.selected_files.insert(file_path.clone());
                                            // Add to context files if not already present
                                            if !self.context_files.iter().any(|f| f.path == *file_path) {
                                                println!("Adding new context file: {}", file_path);
                                                self.context_files.push(ContextFile {
                                                    path: file_path.clone(),
                                                    content: String::new(), // Content should be loaded here
                                                    is_active: true,
                                                });
                                            }
                                        } else {
                                            println!("Removing file from selected files: {}", file_path);
                                            self.selected_files.remove(file_path);
                                            self.context_files.retain(|f| f.path != *file_path);
                                        }
                                    }
                                    ui.label(file_path);
                                });
                            }
                        });
                    
                    // Add debug information at the bottom
                    ui.separator();
                    ui.label(format!("Selected files count: {}", self.selected_files.len()));
                    ui.label(format!("Context files count: {}", self.context_files.len()));
                });
            });
    }
}

fn extract_code_block(text: &str) -> String {
    let markdown_block_pattern: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("```"))
        .skip(1)
        .take_while(|line| !line.starts_with("```"))
        .collect();

    if !markdown_block_pattern.is_empty() {
        return markdown_block_pattern.join("\n");
    }

    let code_lines: Vec<&str> = text
        .lines()
        .filter(|line| 
            line.contains("class ") || 
            line.contains("fun ") || 
            line.contains("import ") || 
            line.contains("{") || 
            line.contains("}") || 
            line.trim().starts_with(".")
        )
        .collect();

    code_lines.join("\n")
}