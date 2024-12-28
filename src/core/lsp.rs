use std::process::Stdio;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::process::{Child, Command};
use tower_lsp::{Client, LanguageServer, LspService, Server};
use lsp_types::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use tower_lsp::jsonrpc::Result as JsonRpcResult;
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

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
        }
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting Kotlin LSP server...");
        let server_path = Path::new("src/resources/server/bin/kotlin-language-server.bat");
        if !server_path.exists() {
            return Err("Kotlin LSP server not found".into());
        }
    
        let mut process = Command::new(server_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
    
        let stdin = process.stdin.take()
            .ok_or("Failed to get stdin")?;
        let stdout = process.stdout.take()
            .ok_or("Failed to get stdout")?;
    
        // Create bidirectional channels for communication
        let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(32);
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(32);

        // Spawn a task to handle stdin
        let stdin_handle = tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(data) = input_rx.recv().await {
                if stdin.write_all(&data).await.is_err() {
                    break;
                }
            }
        });

        // Spawn a task to handle stdout
        let stdout_handle = tokio::spawn(async move {
            let mut stdout = stdout;
            let mut buf = vec![0; 1024];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(n) if n > 0 => {
                        if output_tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        });

        // Create wrapper types that implement AsyncRead and AsyncWrite
        struct AsyncReader {
            rx: mpsc::Receiver<Vec<u8>>,
            buffer: Vec<u8>,
            pos: usize,
        }

        struct AsyncWriter {
            tx: mpsc::Sender<Vec<u8>>,
        }

        impl AsyncRead for AsyncReader {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                use std::task::Poll;

                if self.pos < self.buffer.len() {
                    let remaining = &self.buffer[self.pos..];
                    let amt = std::cmp::min(remaining.len(), buf.remaining());
                    buf.put_slice(&remaining[..amt]);
                    self.pos += amt;
                    return Poll::Ready(Ok(()));
                }

                match self.rx.poll_recv(cx) {
                    Poll::Ready(Some(data)) => {
                        self.buffer = data;
                        self.pos = 0;
                        let amt = std::cmp::min(self.buffer.len(), buf.remaining());
                        buf.put_slice(&self.buffer[..amt]);
                        self.pos += amt;
                        Poll::Ready(Ok(()))
                    }
                    Poll::Ready(None) => Poll::Ready(Ok(())),
                    Poll::Pending => Poll::Pending,
                }
            }
        }

        impl AsyncWrite for AsyncWriter {
            fn poll_write(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                buf: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                let tx = &self.tx;
        
                // Instead of poll_ready, directly attempt to send using try_send
                match tx.try_send(buf.to_vec()) {
                    Ok(_) => std::task::Poll::Ready(Ok(buf.len())),
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => std::task::Poll::Ready(Err(
                        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed"),
                    )),
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => std::task::Poll::Pending,
                }
            }
        
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        }
       
        self.kotlin_server_process = Some(process);
        
        let document_map = Arc::clone(&self.document_map);
        let completion_tx = self.completion_tx.clone();
    
        let (service, socket) = LspService::new(move |client| {
            KotlinLanguageServer::new(
                client,
                document_map.clone(),
                completion_tx.clone(),
            )
        });
    
        let reader = AsyncReader {
            rx: output_rx,
            buffer: Vec::new(),
            pos: 0,
        };
        
        let writer = AsyncWriter {
            tx: input_tx,
        };
    
        let server = tokio::spawn(async move {
            Server::new(reader, writer, socket)
                .serve(service)
                .await;
            
            // Clean up
            stdin_handle.abort();
            stdout_handle.abort();
        });
    
        self.server = Some(server);
        println!("Kotlin LSP server started successfully");
        
        Ok(())
    }

    async fn handle_completion_response(&self, response: CompletionResponse) {
        if let CompletionResponse::Array(items) = response {
            if let Err(e) = self.completion_tx.send(items).await {
                eprintln!("Failed to send completion items: {}", e);
            }
        }
    }

    pub async fn request_completions(&mut self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        let uri = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };

        let content = {
            let documents = self.document_map.lock().await;
            documents.get(&uri).cloned().unwrap_or_default()
        };

        println!("Updating document: {}", uri);
        self.update_document(uri.clone(), content.clone()).await;

        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri.parse()?,
                },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };

        println!("Sending completion request with params: {:?}", params);
        
        // Instead of just sending the request, handle the response
        if let Some(response) = self.get_completions() {
            self.handle_completion_response(CompletionResponse::Array(response)).await;
        }

        Ok(())
    }

    pub fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        match self.completion_rx.try_recv() {
            Ok(completions) => {
                println!("Received completions: {:?}", completions);
                Some(completions)
            }
            Err(_) => None,
        }
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
        
        if let Some(mut process) = self.kotlin_server_process.take() {
            let _ = process.start_kill(); // Using tokio's process kill
        }
    }
}