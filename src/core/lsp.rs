use tower_lsp::{Client, LanguageServer};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, 
    ServerCapabilities, TextDocumentSyncCapability, 
    TextDocumentSyncKind, Diagnostic, DiagnosticSeverity,
    CompletionItem,
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use serde_json::Value;
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader};
use lsp_types::Url;

pub struct KotlinLanguageServer {
    client: Option<Client>,
    documents: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl KotlinLanguageServer {
    pub fn new() -> Self {
        Self {
            client: None,
            documents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
    pub async fn send_diagnostics(&self, uri: String, diagnostics: Vec<Diagnostic>) {
        if let Some(client) = &self.client {
            client.publish_diagnostics(
                Url::parse(&uri).unwrap(), 
                diagnostics, 
                None
            ).await;
        }
    }

    // Example method to generate some sample diagnostics
    fn generate_sample_diagnostics(&self, document_content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Example: Check for unused variables
        if document_content.contains("var unusedVar") {
            diagnostics.push(Diagnostic {
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 10 },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("kotlin-language-server".to_string()),
                message: "Unused variable detected".to_string(),
                ..Default::default()
            });
        }

        // Example: Check for potential syntax errors
        if document_content.contains("if (") && !document_content.contains(")") {
            diagnostics.push(Diagnostic {
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 5 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("kotlin-language-server".to_string()),
                message: "Incomplete if statement".to_string(),
                ..Default::default()
            });
        }

        diagnostics
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut documents = self.documents.lock().await;
        if let Some(change) = params.content_changes.first() {
            let uri = params.text_document.uri.to_string();
            documents.insert(uri.clone(), change.text.clone());

            // Generate and send diagnostics
            let diagnostics = self.generate_sample_diagnostics(&change.text);
            if let Some(client) = &self.client {
                client.publish_diagnostics(
                    params.text_document.uri.clone(), 
                    diagnostics, 
                    None
                ).await;
            }
        }
    }

}

#[tower_lsp::async_trait]
impl LanguageServer for KotlinLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        println!("LSP: Initializing server");
        println!("LSP: Root URI: {:?}", params.root_uri);
        
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), " ".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: Some(tower_lsp::lsp_types::ServerInfo {
                name: "Kotlin Language Server".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        if let Some(client) = &self.client {
            client.log_message(
                tower_lsp::lsp_types::MessageType::INFO, 
                "Kotlin LSP server initialized"
            ).await;
        }
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let mut documents = self.documents.lock().await;
        documents.insert(
            params.text_document.uri.to_string(), 
            params.text_document.text
        );
    }

    // Modify the did_change method to generate and send diagnostics
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut documents = self.documents.lock().await;
        if let Some(change) = params.content_changes.first() {
            let uri = params.text_document.uri.to_string();
            documents.insert(uri.clone(), change.text.clone());

            // Generate and send diagnostics
            let diagnostics = self.generate_sample_diagnostics(&change.text);
            if let Some(client) = &self.client {
                client.publish_diagnostics(
                    params.text_document.uri.clone(), 
                    diagnostics, 
                    None
                ).await;
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        println!("LSP: Completion request received");
        println!("LSP: Document URI: {}", params.text_document_position.text_document.uri);
        println!("LSP: Position - Line: {}, Character: {}", 
            params.text_document_position.position.line, 
            params.text_document_position.position.character
        );

        let documents = self.documents.lock().await;
        let uri = params.text_document_position.text_document.uri.to_string();
        
        let completion_items = if let Some(document_content) = documents.get(&uri) {
            println!("LSP: Found document content, generating completions");
            
            // More sophisticated completion generation
            let mut items = vec![
                tower_lsp::lsp_types::CompletionItem {
                    label: "println()".to_string(),
                    kind: Some(tower_lsp::lsp_types::CompletionItemKind::FUNCTION),
                    detail: Some("Kotlin print function".to_string()),
                    insert_text: Some("println()".to_string()),
                    ..Default::default()
                },
                tower_lsp::lsp_types::CompletionItem {
                    label: "fun".to_string(),
                    kind: Some(tower_lsp::lsp_types::CompletionItemKind::KEYWORD),
                    detail: Some("Function declaration".to_string()),
                    insert_text: Some("fun ${1:functionName}() {\n    $0\n}".to_string()),
                    ..Default::default()
                }
            ];

            // Context-based suggestions
            let current_line = document_content
                .lines()
                .nth(params.text_document_position.position.line as usize)
                .unwrap_or("");
            
            println!("LSP: Current line context: {}", current_line);

            if current_line.contains("import") {
                items.push(tower_lsp::lsp_types::CompletionItem {
                    label: "kotlin.stdlib".to_string(),
                    kind: Some(tower_lsp::lsp_types::CompletionItemKind::MODULE),
                    detail: Some("Kotlin Standard Library".to_string()),
                    ..Default::default()
                });
            }

            items
        } else {
            println!("LSP: No document content found for URI");
            vec![]
        };

        println!("LSP: Generated {} completion items", completion_items.len());

        Ok(Some(CompletionResponse::List(tower_lsp::lsp_types::CompletionList {
            is_incomplete: false,
            items: completion_items,
        })))
    }
}













pub struct LspManager {
    lsp_process: Option<Child>,
    tx: mpsc::Sender<String>,
    rx: Arc<Mutex<mpsc::Receiver<String>>>,
}

impl LspManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            lsp_process: None,
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn start_server(&mut self) -> Result<(), String> {
        let server_path = "src/resources/server/bin/kotlin-language-server.bat";
        let mut process = Command::new(server_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start LSP server: {}", e))?;

        // Take ownership of stdout before moving process
        let child_stdout = process.stdout.take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;

        self.lsp_process = Some(process);

        // Spawn a background thread for handling server responses
        let tx_clone = self.tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(child_stdout);
            for line in reader.lines() {
                if let Ok(msg) = line {
                    // Attempt to parse JSON, but don't require it
                    let _ = serde_json::from_str::<Value>(&msg);
                    
                    // Try to send the message, ignore if channel is full
                    let _ = tx_clone.try_send(msg);
                }
            }
        });

        Ok(())
    }

    pub fn send_request(&self, request: String) {
        let _ = self.tx.try_send(request);
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        // This is a placeholder. In a real implementation, 
        // you'd communicate with the actual LSP server
        Some(vec![
            CompletionItem {
                label: "println()".to_string(),
                kind: Some(tower_lsp::lsp_types::CompletionItemKind::FUNCTION),
                detail: Some("Kotlin print function".to_string()),
                insert_text: Some("println()".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "fun".to_string(),
                kind: Some(tower_lsp::lsp_types::CompletionItemKind::KEYWORD),
                detail: Some("Function declaration".to_string()),
                insert_text: Some("fun functionName() {\n    \n}".to_string()),
                ..Default::default()
            }
        ])
    }

    pub fn get_diagnostics(&mut self) -> Option<Vec<Diagnostic>> {
        // This is a placeholder for sample diagnostics
        Some(vec![
            Diagnostic {
                range: lsp_types::Range {
                    start: lsp_types::Position { line: 0, character: 0 },
                    end: lsp_types::Position { line: 0, character: 10 },
                },
                severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                source: Some("kotlin-language-server".to_string()),
                message: "Sample warning: Potential improvement".to_string(),
                ..Default::default()
            }
        ])
    }
    
    pub fn stop_server(&mut self) {
        if let Some(mut process) = self.lsp_process.take() {
            let _ = process.kill();
        }
    }
}