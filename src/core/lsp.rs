use tokio::sync::{mpsc, Mutex as TokioMutex};
use tower_lsp::{Client, LanguageServer, LspService};
use lsp_types::*;
use lsp_types::request::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::result::Result;
use tower_lsp::jsonrpc::Result as JsonRpcResult;

#[derive(Default)]
struct ContextCache {
    imports: HashMap<String, Vec<String>>,
    declarations: HashMap<String, Vec<Declaration>>,
    project_dependencies: Vec<String>,
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    kind: DeclarationType,
    scope: String,
    location: Range,
}

#[derive(Clone, Debug)]
enum DeclarationType {
    Class,
    Function,
    Variable,
    Property,
}

struct KotlinLanguageServer {
    client: Client,
    document_map: Arc<TokioMutex<HashMap<String, String>>>,
    completion_tx: mpsc::Sender<Vec<CompletionItem>>,
    initialization_status: Arc<TokioMutex<bool>>,
}

impl KotlinLanguageServer {
    fn new(
        client: Client,
        document_map: Arc<TokioMutex<HashMap<String, String>>>,
        completion_tx: mpsc::Sender<Vec<CompletionItem>>,
        initialization_status: Arc<TokioMutex<bool>>,
    ) -> Self {
        Self {
            client,
            document_map,
            completion_tx,
            initialization_status,
        }
    }

    async fn analyze_document(&self, uri: &str) -> Vec<CompletionItem> {
        let documents = self.document_map.lock().await;
        if let Some(content) = documents.get(uri) {
            // Here you would analyze the document content and generate context-aware completions
            // This is a simplified example - you'd want to add proper parsing and analysis
            let mut completions = Vec::new();
            
            // Add local variable completions
            for line in content.lines() {
                if let Some(var_name) = line.trim().strip_prefix("var ") {
                    let var_name = var_name.split(':').next().unwrap_or("").trim().to_string();
                    if !var_name.is_empty() {
                        completions.push(CompletionItem {
                            label: var_name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("Local variable".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }

            // Add function completions
            for line in content.lines() {
                if let Some(fn_def) = line.trim().strip_prefix("fun ") {
                    if let Some(fn_name) = fn_def.split('(').next() {
                        completions.push(CompletionItem {
                            label: fn_name.trim().to_string(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some("Function".to_string()),
                            ..Default::default()
                        });
                    }
                }
            }

            completions
        } else {
            Vec::new()
        }
    }

    fn extract_imports(&self, content: &str) -> Vec<String> {
        content.lines()
            .filter(|line| line.trim().starts_with("import"))
            .map(|line| line.trim().to_string())
            .collect()
    }

    fn parse_declarations(&self, content: &str) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();
            
            // Parse classes
            if line.starts_with("class ") {
                declarations.push(Declaration {
                    name: line.split_whitespace().nth(1)
                        .unwrap_or("").to_string(),
                    kind: DeclarationType::Class,
                    scope: "global".to_string(),
                    location: Range {
                        start: Position { line: i as u32, character: 0 },
                        end: Position { line: i as u32, character: line.len() as u32 },
                    },
                });
            }
            
            // Parse functions
            if line.starts_with("fun ") {
                declarations.push(Declaration {
                    name: line.split_whitespace().nth(1)
                        .unwrap_or("").split('(').next()
                        .unwrap_or("").to_string(),
                    kind: DeclarationType::Function,
                    scope: "global".to_string(),
                    location: Range {
                        start: Position { line: i as u32, character: 0 },
                        end: Position { line: i as u32, character: line.len() as u32 },
                    },
                });
            }
        }
        
        declarations
    }

    async fn analyze_project_dependencies(&self, root: &Url) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let gradle_file = root.join("build.gradle.kts")?;
        if let Ok(content) = tokio::fs::read_to_string(gradle_file.path()).await {
            let deps = content.lines()
                .filter(|line| line.contains("implementation"))
                .map(|line| line.trim().to_string())
                .collect();
            Ok(deps)
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_context_aware_completions(&self, uri: &str, position: Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        if let Some(content) = self.document_map.lock().await.get(uri) {
            // Add basic completions
            self.add_context_based_completions(&mut completions, content);
            
            // Add document-specific completions
            if let Some(line) = content.lines().nth(position.line as usize) {
                let prefix = &line[..position.character as usize];
                
                // Add more specific completions based on context
                if prefix.contains(".") {
                    // Add method completions
                    completions.push(CompletionItem {
                        label: "toString()".to_string(),
                        kind: Some(CompletionItemKind::METHOD),
                        detail: Some("Convert to string".to_string()),
                        ..Default::default()
                    });
                }
            }
        }
        
        // Send completions through channel
        let _ = self.completion_tx.send(completions.clone()).await;
        
        completions
    }

    fn add_context_based_completions(&self, completions: &mut Vec<CompletionItem>, _content: &str) {
        // Add basic keyword completions
        for keyword in ["class", "fun", "val", "var", "if", "when", "for", "while"] {
            completions.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Kotlin keyword".to_string()),
                ..Default::default()
            });
        }
    }

    pub async fn send_diagnostics(&self, uri: String, diagnostics: Vec<Diagnostic>) {
        let client = &self.client;
        client.publish_diagnostics(
            Url::parse(&uri).unwrap(), 
            diagnostics, 
            None
        ).await;
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
    async fn initialize(&self, _: InitializeParams) -> JsonRpcResult<InitializeResult> {
        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL
            )),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(true),
                trigger_characters: Some(vec![".".to_string()]),
                work_done_progress_options: Default::default(),
                all_commit_characters: None,
                completion_item: None,
            }),
            ..Default::default()
        };

        Ok(InitializeResult {
            capabilities,
            server_info: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        *self.initialization_status.lock().await = true;
        self.client.log_message(MessageType::INFO, "Server initialized!").await;
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }

    async fn completion(&self, _params: CompletionParams) -> JsonRpcResult<Option<CompletionResponse>> {
        let items = vec![
            CompletionItem {
                label: "test_completion".to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Test completion item".to_string()),
                ..Default::default()
            }
        ];
        
        Ok(Some(CompletionResponse::Array(items)))
    }
    
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let content = params.text_document.text;
        let mut documents = self.document_map.lock().await;
        documents.insert(uri, content);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        if let Some(change) = params.content_changes.get(0) {
            let mut documents = self.document_map.lock().await;
            documents.insert(uri, change.text.clone());
        }
    }
}













pub struct LspManager {
    document_map: Arc<TokioMutex<HashMap<String, String>>>,
    completion_tx: mpsc::Sender<Vec<CompletionItem>>,
    completion_rx: mpsc::Receiver<Vec<CompletionItem>>,
    server_state: Arc<TokioMutex<Option<LspServerState>>>,
    initialization_status: Arc<TokioMutex<bool>>,
}

struct LspServerState {
    _process: std::process::Child,
    client: Client,
}

impl LspManager {
    pub fn new() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel(32);
        
        Self {
            document_map: Arc::new(TokioMutex::new(HashMap::new())),
            completion_tx,
            completion_rx,
            server_state: Arc::new(TokioMutex::new(None)),
            initialization_status: Arc::new(TokioMutex::new(false)),
        }
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting LSP server...");
        
        let server_path = std::env::current_dir()?.join("src/resources/server/bin/kotlin-language-server.bat");
        println!("Server path: {:?}", server_path);
        
        let process = std::process::Command::new(&server_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        println!("Server process started");

        let initialization_status = Arc::clone(&self.initialization_status);
        
        // Create LSP connection
        let (service, socket) = LspService::new(move |client| {
            let doc_map = Arc::new(TokioMutex::new(HashMap::new()));
            let completion_tx = mpsc::channel(32).0;
            let init_status = Arc::clone(&initialization_status);
            
            KotlinLanguageServer::new(
                client.clone(),
                doc_map,
                completion_tx,
                init_status,
            )
        });

        println!("LSP service created");

        // Store server state
        let mut server_state = self.server_state.lock().await;
        *server_state = Some(LspServerState {
            _process: process,
            client: service.inner().client.clone(),
        });

        // Create async read/write streams
        let (read_half, write_half) = tokio::io::duplex(1024);
        
        // Spawn server in background task
        tokio::spawn(async move {
            println!("Starting LSP server loop");
            tower_lsp::Server::new(read_half, write_half, socket)
                .serve(service)
                .await;
        });

        // Wait for initialization
        let mut retries = 0;
        while !*self.initialization_status.lock().await {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            retries += 1;
            if retries > 50 {  // 5 second timeout
                return Err("Server initialization timeout".into());
            }
        }

        println!("LSP server initialized successfully");
        Ok(())
    }

    pub async fn request_completions(&self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        if !*self.initialization_status.lock().await {
            return Err("Server not initialized".into());
        }

        println!("Requesting completions for uri: {}, position: {:?}", uri, position);
        
        if let Some(server_state) = self.server_state.lock().await.as_ref() {
            let params = CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { 
                        uri: Url::parse(&uri)? 
                    },
                    position,
                },
                context: Some(CompletionContext {
                    trigger_kind: CompletionTriggerKind::INVOKED,
                    trigger_character: None,
                }),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };
    
            println!("Sending completion request to LSP server");
            
            // Use send_request instead of completion method
            let request = lsp_types::request::Completion::METHOD.to_string();
            match server_state.client.send_request::<lsp_types::request::Completion>(params).await {
                Ok(Some(completion_response)) => {
                    match completion_response {
                        CompletionResponse::Array(items) => {
                            println!("Received {} completion items", items.len());
                            let _ = self.completion_tx.send(items).await;
                        },
                        CompletionResponse::List(list) => {
                            println!("Received {} completion items", list.items.len());
                            let _ = self.completion_tx.send(list.items).await;
                        }
                    }
                    Ok(())
                },
                Ok(None) => {
                    println!("No completions available");
                    Ok(())
                },
                Err(e) => Err(format!("Completion request failed: {}", e).into()),
            }
        } else {
            Err("No LSP server state available".into())
        }
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        self.completion_rx.try_recv().ok()
    }

    pub async fn update_document(&self, uri: String, content: String) {
        println!("Updating document: {}", uri);
        let mut documents = self.document_map.lock().await;
        documents.insert(uri, content);
    }

    pub async fn stop_server(&mut self) {
        if let Some(mut state) = self.server_state.lock().await.take() {
            let _ = state._process.kill();
        }
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        println!("Shutting down LSP manager");
        if let Some(mut server_state) = self.server_state.try_lock().ok().and_then(|mut guard| guard.take()) {
            let _ = server_state._process.kill();
        }
    }
}