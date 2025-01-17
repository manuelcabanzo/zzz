use eframe::egui;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::VecDeque;
use chrono::Local;

// Previous structs remain the same...
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
    stop: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TogetherAIResponse {
    output: Output,
}

#[derive(Debug, Clone, Deserialize)]
struct Output {
    text: String,
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
}

impl AIAssistant {
    pub fn new(api_key: String) -> Self {
        println!("Initializing AI Assistant with API key length: {}", api_key.len());
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
        }
    }

    // Previous helper methods remain the same...
    fn add_debug_message(&mut self, msg: String) {
        println!("AI Assistant Debug: {}", msg);
        self.debug_messages.push_back(msg);
        while self.debug_messages.len() > 10 {
            self.debug_messages.pop_front();
        }
    }

    fn format_prompt(&self, file_content: &str, question: &str) -> String {
        let chat_context: String = self.chat_history
            .iter()
            .take(self.context_window)
            .map(|msg| {
                if msg.is_user {
                    format!("Human: {}\n", msg.content)
                } else {
                    format!("Assistant: {}\n", msg.content)
                }
            })
            .collect();

        format!(
            "You are an AI programming assistant in an IDE. You have access to the current file content.\n\
            Current File Content:\n{}\n\n\
            Previous conversation:\n{}\n\
            Human: {}\n\
            Assistant:",
            file_content,
            chat_context,
            question
        )
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
                        ui.colored_label(egui::Color32::RED, 
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
                                        let (bg_color, text_color) = if message.is_user {
                                            (ui.style().visuals.extreme_bg_color, ui.style().visuals.text_color())
                                        } else {
                                            (ui.style().visuals.code_bg_color, egui::Color32::LIGHT_BLUE)
                                        };

                                        egui::Frame::none()
                                            .fill(bg_color)
                                            .outer_margin(egui::vec2(8.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(&message.timestamp)
                                                        .small()
                                                        .color(egui::Color32::GRAY));
                                                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                                        ui.label(egui::RichText::new(&message.content)
                                                            .color(text_color));
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
                            [ui.available_width() - 80.0, 80.0],
                            egui::TextEdit::multiline(&mut self.input_text)
                                .hint_text("Ask about your code or request changes...")
                                .desired_rows(3)
                        );

                        ui.vertical(|ui| {
                            ui.add_space(20.0);
                            let send_button = ui.add_sized(
                                [70.0, 40.0],
                                egui::Button::new(
                                    egui::RichText::new(if self.is_loading { "⌛" } else { "Send" })
                                        .size(16.0)
                                )
                            );

                            if (text_edit.lost_focus() && 
                                ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) || 
                                send_button.clicked()) && 
                                !self.input_text.trim().is_empty() && 
                                !self.is_loading 
                            {
                                let question = std::mem::take(&mut self.input_text);
                                self.add_message(question.clone(), true);
                                self.is_loading = true;
                                
                                // API call setup
                                let file_content = code_editor.get_active_content();
                                let prompt = self.format_prompt(&file_content, &question);
                                
                                let tx = self.tx.clone();
                                let api_key = self.api_key.clone();
                                let client = self.http_client.clone();

                                tokio::spawn(async move {
                                    let request = TogetherAIRequest {
                                        model: "togethercomputer/CodeLlama-34b-Instruct".to_string(),
                                        prompt: prompt.clone(),
                                        max_tokens: 2048,
                                        temperature: 0.7,
                                        stop: vec!["Human:".to_string(), "\nHuman:".to_string()],
                                    };

                                    match client
                                        .post("https://api.together.xyz/inference")
                                        .header("Authorization", format!("Bearer {}", api_key))
                                        .json(&request)
                                        .send()
                                        .await
                                    {
                                        Ok(response) => {
                                            match response.error_for_status() {
                                                Ok(response) => {
                                                    match response.json::<TogetherAIResponse>().await {
                                                        Ok(ai_response) => {
                                                            let _ = tx.send(ai_response.output.text.trim().to_string()).await;
                                                        }
                                                        Err(e) => {
                                                            let _ = tx.send(format!("Error parsing response: {}", e)).await;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx.send(format!("API error: {}", e)).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let _ = tx.send(format!("Network error: {}", e)).await;
                                        }
                                    }
                                });
                            }
                        });
                    });

                    // Handle responses
                    while let Ok(response) = self.rx.try_recv() {
                        if response.starts_with("Error") || 
                           response.starts_with("Network error") || 
                           response.starts_with("API error") 
                        {
                            self.add_debug_message(response.clone());
                        }
                        self.add_message(response, false);
                        self.is_loading = false;
                    }
                });
            });
    }
}