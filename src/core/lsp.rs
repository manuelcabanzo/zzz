use tokio::sync::{mpsc, Mutex as TokioMutex};
use tower_lsp::{Client, LanguageServer, LspService, Server};
use lsp_types::*;
use std::sync::Arc;
use std::collections::HashMap;
use tower_lsp::jsonrpc::Result as JsonRpcResult;

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
}

impl LspManager {
    pub fn new() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel(32);
        
        Self {
            document_map: Arc::new(TokioMutex::new(HashMap::new())),
            completion_tx,
            completion_rx,
            server: None,
        }
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting LSP server...");
        
        let document_map = Arc::clone(&self.document_map);
        let completion_tx = self.completion_tx.clone();

        let (service, socket) = LspService::new(move |client| {
            KotlinLanguageServer::new(
                client,
                document_map.clone(),
                completion_tx.clone(),
            )
        });

        let server = tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();
            
            Server::new(stdin, stdout, socket)
                .serve(service)
                .await;
        });

        self.server = Some(server);
        println!("LSP server started successfully");
        
        Ok(())
    }

    pub async fn request_completions(&self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        // Create proper URI
        let uri = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };

        let content = "".to_string(); // You should get the actual content here
        self.update_document(uri.clone(), content).await;

        println!("Requesting completions for uri: {}, position: {:?}", uri, position);
        Ok(())
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        self.completion_rx.try_recv().ok()
    }

    pub async fn update_document(&self, uri: String, content: String) {
        println!("Updating document: {}", uri);
        let mut documents = self.document_map.lock().await;
        documents.insert(uri, content);
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        println!("Shutting down LSP manager");
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }
}