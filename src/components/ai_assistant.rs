use eframe::egui;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::VecDeque;
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
    model: String, // Add field for AI model
}

impl AIAssistant {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_MS: u64 = 1000;

    pub fn new(api_key: String, runtime: Arc<Runtime>) -> Self {
        let (tx, rx) = mpsc::channel(32);
        
        Self {
            api_key,
            input_text: String::new(),
            chat_history: VecDeque::with_capacity(100),
            is_loading: false,
            http_client: Client::new(),
            tx,
            rx,
            scroll_to_bottom: false,
            context_window: 5,
            debug_messages: VecDeque::with_capacity(10),
            panel_height: 600.0,
            runtime,
            last_ai_response: None,
            model: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(), // Default model
        }
    }

    pub fn update_model(&mut self, new_model: String) {
        self.model = new_model;
    }

    fn format_chat_messages(&self, file_content: &str, question: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();
        
        // System message to set the context
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: format!(
                "You are an AI programming assistant in an IDE. You have access to the current file content:\n```\n{}\n```\n\
                Be direct and concise. Focus on practical solutions.",
                file_content
            ),
        });

        // Add previous conversation context
        for msg in self.chat_history.iter().take(self.context_window) {
            messages.push(ChatMessage {
                role: if msg.is_user { "user" } else { "assistant" }.to_string(),
                content: msg.content.clone(),
            });
        }

        // Add the current question
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: question.to_string(),
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
        while self.debug_messages.len() > 10 {
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
        
        while self.chat_history.len() > 100 {
            self.chat_history.pop_front();
        }
        
        self.scroll_to_bottom = true;
    }

    pub fn update_api_key(&mut self, new_key: String) {
        self.api_key = new_key;
    }
    
    pub fn show(&mut self, ui: &mut egui::Ui, code_editor: &mut super::code_editor::CodeEditor) {
        let available_height = ui.available_height();
        self.panel_height = available_height;
    
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
                    
                    // Debug section
                    egui::CollapsingHeader::new("Debug Info")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(format!("API Key length: {}", self.api_key.len()));
                            ui.label(format!("Is loading: {}", self.is_loading));
                            ui.label(format!("Chat history length: {}", self.chat_history.len()));
                            
                            ui.label("Recent debug messages:");
                            for msg in &self.debug_messages {
                                ui.label(msg);
                            }
                        });
    
                    if self.api_key.is_empty() {
                        ui.colored_label(
                            egui::Color32::RED, 
                            "Please configure your Together AI API key in Settings"
                        );
                        return;
                    }
    
                    ui.add_space(8.0);
    
                    // Chat area
                    let chat_height = self.panel_height - 200.0;
                    egui::Frame::none()
                        .fill(ui.style().visuals.extreme_bg_color)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(chat_height)
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
                                                    
                                                    // Using correct enum value Align::LEFT
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
                        });
    
                    ui.add_space(8.0);
    
                    // Input area
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

                            // "Send" button
                            let send_button = ui.add_sized(
                                [70.0, 40.0],
                                egui::Button::new(
                                    egui::RichText::new(if self.is_loading { "⌛" } else { "Send" })
                                        .size(16.0)
                                )
                            );

                            // Only show the "Apply Code" button if there's a code block
                            if has_code_block {
                                ui.vertical(|ui| {

                                    // "Apply Code" button
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
                                let model = self.model.clone(); // Use the selected model

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
    
                    while let Ok(response) = self.rx.try_recv() {
                        if response.starts_with("Error") || 
                           response.starts_with("Network error") || 
                           response.starts_with("API error") 
                        {
                            self.add_debug_message(response.clone());
                        }
                        
                        // Store the last AI response for potential code application
                        self.last_ai_response = Some(response.clone());
                        
                        self.add_message(response.clone(), false);
                        self.is_loading = false;
                    }
                });
            });
    }
}

fn extract_code_block(text: &str) -> String {
    // First, try to extract markdown code block
    let markdown_block_pattern: Vec<&str> = text
        .lines()
        .skip_while(|line| !line.starts_with("```"))
        .skip(1)  // Skip the opening ```
        .take_while(|line| !line.starts_with("```"))
        .collect();

    if !markdown_block_pattern.is_empty() {
        return markdown_block_pattern.join("\n");
    }

    // If no markdown block, try to extract the entire code portion
    let code_lines: Vec<&str> = text
        .lines()
        .filter(|line| 
            // Basic heuristics to identify code-like lines
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