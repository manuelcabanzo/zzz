use eframe::egui;
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use reqwest::Client;

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
    response_text: String,
    is_loading: bool,
    http_client: Client,
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
}

impl AIAssistant {
    pub fn new(api_key: String) -> Self {
        let (tx, rx) = mpsc::channel(32);
        
        Self {
            api_key,
            input_text: String::new(),
            response_text: String::new(),
            is_loading: false,
            http_client: Client::new(),
            tx,
            rx,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, code_editor: &mut super::code_editor::CodeEditor) {
        ui.vertical(|ui| {
            ui.heading("AI Assistant");
            
            if self.api_key.is_empty() {
                ui.label("Please configure your Together AI API key in Settings > AI Assistant");
                return;
            }
            
            // Input area
            ui.text_edit_multiline(&mut self.input_text);
            
            ui.horizontal(|ui| {
                if ui.button("Ask AI").clicked() && !self.is_loading {
                    self.is_loading = true;
                    
                    // Get current file context
                    let context = code_editor.get_active_content();
                    
                    // Create prompt with context
                    let prompt = format!(
                        "Given this code context:\n\n{}\n\nQuestion: {}\n\nAnswer:",
                        context,
                        self.input_text
                    );
                    
                    // Clone values for async closure
                    let tx = self.tx.clone();
                    let api_key = self.api_key.clone();
                    let client = self.http_client.clone();
                    
                    // Spawn async task
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
                
                if self.is_loading {
                    ui.spinner();
                }
            });
            
            // Check for response
            if let Ok(response) = self.rx.try_recv() {
                self.response_text = response;
                self.is_loading = false;
            }
            
            // Display response
            if !self.response_text.is_empty() {
                ui.separator();
                ui.label("AI Response:");
                ui.text_edit_multiline(&mut self.response_text.clone());
                
                if ui.button("Insert at Cursor").clicked() {
                    if let Some(buffer) = code_editor.get_active_buffer_mut() {
                        buffer.content.push_str(&self.response_text);
                        buffer.is_modified = true;
                    }
                }
            }
        });
    }
}