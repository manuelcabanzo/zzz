use tower_lsp::{Client, LanguageServer};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, 
    ServerCapabilities, TextDocumentSyncCapability, 
    TextDocumentSyncKind, Diagnostic, DiagnosticSeverity,
    CompletionItem, Position, Url, CompletionItemKind,
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader};

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
    stdout_tx: mpsc::Sender<String>,
    _stdout_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    
    // Channels for completions
    completion_tx: mpsc::Sender<Vec<CompletionItem>>,
    completion_rx: mpsc::Receiver<Vec<CompletionItem>>,
    
    // Channels for diagnostics
    diagnostic_tx: mpsc::Sender<Vec<Diagnostic>>,
    diagnostic_rx: mpsc::Receiver<Vec<Diagnostic>>,
}

impl LspManager {
    pub fn new() -> Self {
        // Create channels for stdout, completions, and diagnostics
        let (stdout_tx, stdout_rx) = mpsc::channel(100);
        let (completion_tx, completion_rx) = mpsc::channel(10);
        let (diagnostic_tx, diagnostic_rx) = mpsc::channel(10);

        Self {
            lsp_process: None,
            stdout_tx,
            _stdout_rx: Arc::new(Mutex::new(stdout_rx)),
            completion_tx,
            completion_rx,
            diagnostic_tx,
            diagnostic_rx,
        }
    }

    pub fn start_server(&mut self) -> Result<(), String> {
        let server_path = "src/resources/server/bin/kotlin-language-server.bat";
        let mut process = Command::new(server_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start LSP server: {}", e))?;

        let child_stdout = process.stdout.take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let child_stderr = process.stderr.take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        self.lsp_process = Some(process);

        // Clone sender channels for async tasks
        let stdout_tx_clone = self.stdout_tx.clone();


        tokio::spawn(async move {
            let stdout_reader = BufReader::new(child_stdout);
            let stderr_reader = BufReader::new(child_stderr);

            // Read both stdout and stderr
            for line in stdout_reader.lines().chain(stderr_reader.lines()) {
                if let Ok(msg) = line {
                    println!("LSP Server Message: {}", msg);
                    let _ = stdout_tx_clone.try_send(msg);
                }
            }
        });

        Ok(())
    }

    pub async fn request_completions(&mut self, uri: String, position: Position) -> Option<Vec<CompletionItem>> {
        let completions = self.generate_completions(uri, position);
        
        match self.completion_tx.try_send(completions.clone()) {
            Ok(_) => {
                println!("LSP: Successfully sent completions");
                Some(completions)
            },
            Err(e) => {
                eprintln!("LSP: Failed to send completions: {}", e);
                None
            }
        }
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        match self.completion_rx.try_recv() {
            Ok(completions) => {
                if completions.is_empty() {
                    println!("LSP: No completions available");
                    None
                } else {
                    println!("LSP: Retrieved {} completions", completions.len());
                    Some(completions)
                }
            },
            Err(e) => {
                if !matches!(e, mpsc::error::TryRecvError::Empty) {
                    eprintln!("LSP: Error receiving completions: {}", e);
                }
                None
            }
        }
    }


    pub async fn request_diagnostics(&mut self, uri: String, document_content: &str) -> Option<Vec<Diagnostic>> {
        // Generate diagnostics based on the document content
        let diagnostics = self.generate_diagnostics(uri, document_content);
        
        // Send diagnostics through the channel
        let _ = self.diagnostic_tx.send(diagnostics.clone()).await;
        
        Some(diagnostics)
    }

    pub fn get_diagnostics(&mut self) -> Option<Vec<Diagnostic>> {
        // Non-blocking attempt to receive diagnostics
        match self.diagnostic_rx.try_recv() {
            Ok(diagnostics) => {
                println!("LspManager: Retrieved {} diagnostics", diagnostics.len());
                Some(diagnostics)
            },
            Err(_) => {
                println!("LspManager: No diagnostics available");
                None
            }
        }
    }

    fn generate_completions(&self, _uri: String, position: Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        // Add basic completions with proper error handling
        let basic_items = vec![
            ("println", "println()", "Print to console", CompletionItemKind::FUNCTION),
            ("fun", "fun ${1:name}() {\n    $0\n}", "Function declaration", CompletionItemKind::KEYWORD),
            ("class", "class ${1:Name} {\n    $0\n}", "Class declaration", CompletionItemKind::CLASS),
            ("var", "var ${1:name}: ${2:Type} = $0", "Variable declaration", CompletionItemKind::VARIABLE),
        ];

        for (label, insert_text, detail, kind) in basic_items {
            completions.push(CompletionItem {
                label: label.to_string(),
                kind: Some(kind),
                detail: Some(detail.to_string()),
                insert_text: Some(insert_text.to_string()),
                ..Default::default()
            });
        }

        // Log completion generation
        println!("LSP: Generated {} completions at position line: {}, character: {}", 
            completions.len(), position.line, position.character);

        completions
    }

    fn generate_diagnostics(&self, _uri: String, document_content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Example: Check for unused variables
        if document_content.contains("var unusedVar") {
            diagnostics.push(Diagnostic {
                range: lsp_types::Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 10 },
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
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 5 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("kotlin-language-server".to_string()),
                message: "Incomplete if statement".to_string(),
                ..Default::default()
            });
        }

        diagnostics
    }

    pub fn send_request(&self, request: String) {
        let _ = self.stdout_tx.try_send(request);
    }
    
    pub fn stop_server(&mut self) {
        if let Some(mut process) = self.lsp_process.take() {
            let _ = process.kill();
        }
    }
}