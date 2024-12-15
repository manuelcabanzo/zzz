use tower_lsp::{Client, LanguageServer};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, 
    ServerCapabilities, TextDocumentSyncCapability, 
    TextDocumentSyncKind,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::process::{Command, Child, Stdio};
use std::path::PathBuf;
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

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut documents = self.documents.lock().await;
        if let Some(change) = params.content_changes.first() {
            documents.insert(
                params.text_document.uri.to_string(), 
                change.text.clone()
            );
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
    server_process: Option<Child>,
    lsp_path: PathBuf,
}
impl LspManager {
    pub fn new() -> Self {
        println!("LspManager: Initializing LSP Manager");
        Self {
            server_process: None,
            lsp_path: PathBuf::from("src/resources/server/bin/kotlin-language-server.bat"),
        }
    }

    pub fn start_server(&mut self) -> std::io::Result<()> {
        println!("LspManager: Attempting to start LSP server from path: {:?}", self.lsp_path);
        
        match Command::new(&self.lsp_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn() {
                Ok(mut process) => {
                    println!("LspManager: LSP server started successfully");
                    
                    // Capture stdout and stderr
                    let stdout = process.stdout.take().unwrap();
                    let stderr = process.stderr.take().unwrap();
                    
                    // Spawn threads for stdout and stderr logging
                    std::thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                println!("LSP Server STDOUT: {}", line);
                            }
                        }
                    });

                    std::thread::spawn(move || {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(line) = line {
                                eprintln!("LSP Server STDERR: {}", line);
                            }
                        }
                    });

                    self.server_process = Some(process);
                    Ok(())
                },
                Err(e) => {
                    eprintln!("LspManager: Failed to start LSP server. Error: {}", e);
                    Err(e)
                }
            }
    }

    pub fn stop_server(&mut self) {
        println!("LspManager: Attempting to stop LSP server");
        if let Some(mut process) = self.server_process.take() {
            match process.kill() {
                Ok(_) => println!("LspManager: LSP server stopped successfully"),
                Err(e) => eprintln!("LspManager: Error stopping LSP server: {}", e),
            }
        } else {
            println!("LspManager: No active LSP server to stop");
        }
    }
}