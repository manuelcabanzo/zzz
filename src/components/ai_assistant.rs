use eframe::egui;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::VecDeque;
use chrono::Local;

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
}

impl AIAssistant {
    pub fn new(api_key: String) -> Self {
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
        }
    }

    pub fn update_api_key(&mut self, new_key: String) {
        self.api_key = new_key;
    }

    fn format_prompt(&self, context: &str, question: &str) -> String {
        let chat_context: String = self.chat_history
            .iter()
            .take(5) // Only include last 5 messages for context
            .map(|msg| {
                if msg.is_user {
                    format!("User: {}\n", msg.content)
                } else {
                    format!("Assistant: {}\n", msg.content)
                }
            })
            .collect();

        format!(
            "Previous conversation:\n{}\n\nCurrent code context:\n{}\n\nUser: {}\n\nAssistant:",
            chat_context,
            context,
            question
        )
    }

    fn add_message(&mut self, content: String, is_user: bool) {
        let timestamp = Local::now().format("%H:%M").to_string();
        self.chat_history.push_back(Message {
            content,
            is_user,
            timestamp,
        });
        
        // Keep only the last 100 messages
        while self.chat_history.len() > 100 {
            self.chat_history.pop_front();
        }
        
        self.scroll_to_bottom = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, code_editor: &mut super::code_editor::CodeEditor) {
        ui.vertical(|ui| {
            ui.heading("AI Assistant");
            
            if self.api_key.is_empty() {
                ui.label("Please configure your Together AI API key in Settings > AI Assistant");
                return;
            }

            // Chat history area
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(self.scroll_to_bottom)
                .show(ui, |ui| {
                    for message in &self.chat_history {
                        ui.horizontal(|ui| {
                            let text_color = if message.is_user {
                                ui.style().visuals.text_color()
                            } else {
                                egui::Color32::LIGHT_BLUE
                            };

                            ui.label(egui::RichText::new(&message.timestamp).small());
                            ui.label(egui::RichText::new(&message.content).color(text_color));
                        });
                        ui.add_space(4.0);
                    }
                });
            
            self.scroll_to_bottom = false;

            // Input area with send button
            ui.horizontal(|ui| {
                let text_edit = ui.add_sized(
                    [ui.available_width() - 60.0, 60.0],
                    egui::TextEdit::multiline(&mut self.input_text)
                        .hint_text("Type your message here...")
                );

                let send_button = ui.add_enabled(
                    !self.input_text.trim().is_empty() && !self.is_loading,
                    egui::Button::new(if self.is_loading { "⌛" } else { "Send" })
                );

                if (text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                    || send_button.clicked()) && !self.input_text.trim().is_empty() && !self.is_loading 
                {
                    let question = std::mem::take(&mut self.input_text);
                    self.add_message(question.clone(), true);
                    self.is_loading = true;

                    let context = code_editor.get_active_content();
                    let prompt = self.format_prompt(&context, &question);

                    let tx = self.tx.clone();
                    let api_key = self.api_key.clone();
                    let client = self.http_client.clone();

                    tokio::spawn(async move {
                        let request = TogetherAIRequest {
                            model: "togethercomputer/CodeLlama-34b-Instruct".to_string(),
                            prompt,
                            max_tokens: 1024,
                            temperature: 0.7,
                        };

                        let result = async {
                            let response = client
                                .post("https://api.together.xyz/inference")
                                .header("Authorization", format!("Bearer {}", api_key))
                                .json(&request)
                                .send()
                                .await?
                                .error_for_status()?
                                .json::<TogetherAIResponse>()
                                .await?;
                            Ok::<_, reqwest::Error>(response)
                        }.await;

                        match result {
                            Ok(response) => {
                                let _ = tx.send(response.output.text).await;
                            }
                            Err(e) => {
                                let error_msg = format!("Error: {}", e);
                                let _ = tx.send(error_msg).await;
                            }
                        }
                    });
                }
            });

            // Check for new responses
            if let Ok(response) = self.rx.try_recv() {
                self.add_message(response.clone(), false);
                self.is_loading = false;

                // Add "Insert Code" button if response contains code blocks
                if response.contains("```") {
                    ui.horizontal(|ui| {
                        if ui.button("Insert Code at Cursor").clicked() {
                            if let Some(buffer) = code_editor.get_active_buffer_mut() {
                                // Extract code from between ``` markers
                                let code = response
                                    .split("```")
                                    .skip(1)
                                    .step_by(2)
                                    .collect::<Vec<&str>>()
                                    .join("\n");
                                buffer.content.push_str(&code);
                                buffer.is_modified = true;
                            }
                        }
                    });
                }
            }
        });
    }
}