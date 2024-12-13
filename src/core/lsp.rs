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
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string(), " ".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            server_info: None,
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

    async fn completion(&self, _params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        // Placeholder completion - replace with actual Kotlin-specific completions
        Ok(Some(CompletionResponse::List(tower_lsp::lsp_types::CompletionList {
            is_incomplete: false,
            items: vec![
                tower_lsp::lsp_types::CompletionItem {
                    label: "kotlinFunction".to_string(),
                    kind: Some(tower_lsp::lsp_types::CompletionItemKind::FUNCTION),
                    detail: Some("Example Kotlin function".to_string()),
                    ..Default::default()
                }
            ],
        })))
    }
}

pub struct LspManager {
    server_process: Option<Child>,
    lsp_path: PathBuf,
}

impl LspManager {
    pub fn new() -> Self {
        // Adjust this path to where you download the Kotlin Language Server
        Self {
            server_process: None,
            lsp_path: PathBuf::from("src/resources/server/bin/kotlin-language-server.bat"),
        }
    }

    pub async fn initialize(
        &mut self, 
        log_callback: Option<impl Fn(String)>
    ) -> std::result::Result<(), String> {
        // Convert std::io::Error to String for error handling
        match self.start_server() {
            Ok(_) => {
                if let Some(log_fn) = log_callback {
                    log_fn("LSP Server started successfully".to_string());
                }
                Ok(())
            },
            Err(e) => Err(format!("Failed to start LSP server: {}", e))
        }
    }

    pub fn start_server(&mut self) -> std::io::Result<()> {
        self.server_process = Some(
            Command::new(self.lsp_path.join("bin/kotlin-language-server"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?
        );

        // Optional: Start a thread to read server output
        if let Some(process) = &mut self.server_process {
            let stdout = process.stdout.take().unwrap();
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        println!("LSP Server: {}", line);
                    }
                }
            });
        }

        Ok(())
    }

    pub fn stop_server(&mut self) {
        if let Some(mut process) = self.server_process.take() {
            let _ = process.kill();
        }
    }
}