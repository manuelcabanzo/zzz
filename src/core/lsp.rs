use tokio::io::{AsyncWriteExt, AsyncReadExt, AsyncBufReadExt};
use tower_lsp::{Client, LanguageServer};
use lsp_types::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::process::Stdio;
use tower_lsp::jsonrpc::Result as JsonRpcResult;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;

struct KotlinLanguageServer {
    client: Client,
    document_map: Arc<TokioMutex<HashMap<String, String>>>,
    completion_tx: mpsc::Sender<Vec<CompletionItem>>,
}

impl KotlinLanguageServer {
    fn new(
        client: Client,

        document_map: Arc<TokioMutex<HashMap<String, String>>>,
        completion_tx: mpsc::Sender<Vec<CompletionItem>>,
    ) -> Self {
        Self {
            client,
            document_map,
            completion_tx,
        }
    }

    async fn get_completions(&self, uri: &str, position: &Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        if let Some(content) = self.document_map.lock().await.get(uri) {
            // Add basic Kotlin keywords
            let keywords = vec!["class", "fun", "val", "var", "if", "when", "for", "while"];
            for keyword in keywords {
                completions.push(CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("Kotlin keyword".to_string()),
                    ..Default::default()
                });
            }

            // Add context-aware completions based on document content
            if let Some(line) = content.lines().nth(position.line as usize) {
                let prefix = &line[..position.character as usize];
                
                // Add method completions when typing after a dot
                if prefix.ends_with('.') {
                    completions.extend(vec![
                        CompletionItem {
                            label: "toString()".to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some("Convert to string".to_string()),
                            ..Default::default()
                        },
                        CompletionItem {
                            label: "hashCode()".to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some("Get hash code".to_string()),
                            ..Default::default()
                        },
                    ]);
                }

                // Add local variables and functions
                for line in content.lines() {
                    if let Some(var_name) = line.trim().strip_prefix("var ") {
                        completions.push(CompletionItem {
                            label: var_name.split(':').next().unwrap_or("").trim().to_string(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("Local variable".to_string()),
                            ..Default::default()
                        });
                    }
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
            }
        }

        completions
    }
}












#[tower_lsp::async_trait]
impl LanguageServer for KotlinLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> JsonRpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
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
            },
            server_info: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Server initialized!").await;
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }

    async fn completion(&self, params: CompletionParams) -> JsonRpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        
        let completions = self.get_completions(&uri, &position).await;
        let _ = self.completion_tx.send(completions.clone()).await;
        
        Ok(Some(CompletionResponse::Array(completions)))
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
    server: Option<tokio::task::JoinHandle<()>>,
    kotlin_server_process: Option<Child>,
    stdin_tx: Option<mpsc::Sender<String>>,
}

impl LspManager {
    pub fn new() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel(32);
        
        Self {
            document_map: Arc::new(TokioMutex::new(HashMap::new())),
            completion_tx,
            completion_rx,
            server: None,
            kotlin_server_process: None,
            stdin_tx: None,
        }
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting Kotlin LSP server...");
        
        let server_path = std::env::current_dir()?.join("src/resources/server/bin/kotlin-language-server.bat");
        if !server_path.exists() {
            return Err(format!("Kotlin LSP server not found at: {}", server_path.display()).into());
        }

        let mut process = Command::new(&server_path)
            .current_dir(server_path.parent().unwrap())
            .env("JAVA_HOME", "C:\\Program Files\\Java\\jdk-17")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = process.stdin.take().ok_or("Failed to get stdin")?;
        let mut stdout = process.stdout.take().ok_or("Failed to get stdout")?;
        let stderr = process.stderr.take().ok_or("Failed to get stderr")?;

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);
        self.stdin_tx = Some(stdin_tx.clone());

        // Handle stdin
        let mut stdin = stdin;
        tokio::spawn(async move {
            while let Some(message) = stdin_rx.recv().await {
                if let Err(e) = stdin.write_all(message.as_bytes()).await {
                    eprintln!("Error writing to stdin: {}", e);
                    continue;
                }
                if let Err(e) = stdin.flush().await {
                    eprintln!("Error flushing stdin: {}", e);
                }
            }
        });

        self.kotlin_server_process = Some(process);
        println!("LSP Server started, initializing...");
        
        // Initialize the server immediately after starting
        self.initialize_server().await?;
        
        println!("LSP Server initialized successfully");
        Ok(())
    }

    async fn initialize_server(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Send initialize request with more capabilities
        self.send_message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": null,
                "capabilities": {
                    "workspace": {
                        "applyEdit": true,
                        "workspaceEdit": {
                            "documentChanges": true
                        },
                        "didChangeConfiguration": {
                            "dynamicRegistration": true
                        },
                        "didChangeWatchedFiles": {
                            "dynamicRegistration": true
                        },
                        "symbol": {
                            "dynamicRegistration": true
                        },
                        "executeCommand": {
                            "dynamicRegistration": true
                        }
                    },
                    "textDocument": {
                        "synchronization": {
                            "dynamicRegistration": true,
                            "willSave": true,
                            "willSaveWaitUntil": true,
                            "didSave": true
                        },
                        "completion": {
                            "dynamicRegistration": true,
                            "completionItem": {
                                "snippetSupport": true,
                                "commitCharactersSupport": true,
                                "documentationFormat": ["markdown", "plaintext"],
                                "deprecatedSupport": true,
                                "preselectSupport": true
                            },
                            "completionItemKind": {
                                "valueSet": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25]
                            },
                            "contextSupport": true
                        }
                    }
                },
                "trace": "verbose"
            }
        })).await?;

        // Wait a bit for the server to process the initialize request
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Send initialized notification
        self.send_message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        })).await?;

        Ok(())
    }

    async fn handle_completion_response(&self, response: CompletionResponse) {
        if let CompletionResponse::Array(items) = response {
            if let Err(e) = self.completion_tx.send(items).await {
                eprintln!("Failed to send completion items: {}", e);
            }
        }
    }

    async fn send_message(&self, message: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(stdin_tx) = &self.stdin_tx {
            let msg = serde_json::to_string(&message)?;
            let content = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
            println!("Sending message: {}", content); // Debug print
            stdin_tx.send(content).await?;
        }
        Ok(())
    }

    pub async fn request_completions(&self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        // First ensure the document is opened
        let document_content = self.document_map.lock().await.get(&uri).cloned().unwrap_or_default();
        
        // Send didOpen notification
        self.send_message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri.clone(),
                    "languageId": "kotlin",
                    "version": 1,
                    "text": document_content
                }
            }
        })).await?;

        // Wait a bit for the server to process the document
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Request completions
        self.send_message(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/completion",
            "params": {
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": position.line,
                    "character": position.character
                },
                "context": {
                    "triggerKind": 1,
                    "triggerCharacter": "."
                }
            }
        })).await?;

        Ok(())
    }

    async fn send_lsp_message(&self, stdin: &mut tokio::process::ChildStdin, message: serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        let msg = serde_json::to_string(&message)?;
        let content = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
        stdin.write_all(content.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        // Try to receive completions without blocking
        match self.completion_rx.try_recv() {
            Ok(completions) => {
                println!("Received completions: {:?}", completions);
                Some(completions)
            }
            Err(e) => {
                println!("No completions available: {:?}", e);
                None
            }
        }
    }

    pub async fn update_document(&self, uri: String, content: String) {
        println!("Updating document: {} with content length: {}", uri, content.len());
        let mut documents = self.document_map.lock().await;
        documents.insert(uri.clone(), content.clone());
        
        // Send didChange notification with range information
        if let Err(e) = self.send_message(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "version": 1
                },
                "contentChanges": [{
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 999999, "character": 999999}
                    },
                    "rangeLength": 999999,
                    "text": content
                }]
            }
        })).await {
            eprintln!("Error sending didChange notification: {}", e);
        }
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        println!("Shutting down LSP manager");
        if let Some(server) = self.server.take() {
            server.abort();
        }
        
        if let Some(mut process) = self.kotlin_server_process.take() {
            let _ = process.start_kill(); // Using tokio's process kill
        }
    }
}