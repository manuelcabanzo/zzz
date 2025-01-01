use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::process::Child;
use tower_lsp::{Client, LanguageServer, LspService};
use lsp_types::*;
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
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

    async fn handle_completion_request(&self, params: CompletionParams) -> JsonRpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        println!("Processing completion request for {} at {:?}", uri, position);
        
        let completions = self.get_kotlin_completions(&uri, &position).await;
        println!("Generated {} completion items", completions.len());
        
        if completions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(completions)))
        }
    }

    async fn get_kotlin_completions(&self, uri: &str, position: &Position) -> Vec<CompletionItem> {
        println!("Generating completions for position: {:?}", position);
        let mut completions = Vec::new();
        
        if let Some(content) = self.document_map.lock().await.get(uri) {
            // Basic Kotlin keywords
            let keywords = vec![
                ("class", "Define a class"),
                ("fun", "Define a function"),
                ("val", "Immutable variable"),
                ("var", "Mutable variable"),
                ("if", "Conditional statement"),
                ("when", "Pattern matching"),
                ("for", "Loop construct"),
                ("while", "Loop construct"),
            ];
            
            for (keyword, detail) in keywords {
                completions.push(CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(detail.to_string()),
                    insert_text: Some(keyword.to_string()),
                    documentation: Some(Documentation::String(format!("Kotlin {}", detail))),
                    ..Default::default()
                });
            }

            // Context-aware completions
            if let Some(line) = content.lines().nth(position.line as usize) {
                let prefix = &line[..position.character as usize];
                
                // Add method completions after dot
                if prefix.ends_with('.') {
                    let methods = vec![
                        ("toString()", "Convert to string representation"),
                        ("hashCode()", "Get hash code value"),
                        ("equals(other)", "Compare with another object"),
                    ];
                    
                    for (method, detail) in methods {
                        completions.push(CompletionItem {
                            label: method.to_string(),
                            kind: Some(CompletionItemKind::METHOD),
                            detail: Some(detail.to_string()),
                            insert_text: Some(method.to_string()),
                            ..Default::default()
                        });
                    }
                }

                // For Gradle files, add specific completions
                if uri.ends_with("build.gradle.kts") {
                    let gradle_items = vec![
                        ("plugins {}", "Configure Gradle plugins"),
                        ("dependencies {}", "Configure project dependencies"),
                        ("repositories {}", "Configure dependency repositories"),
                        ("android {}", "Configure Android build settings"),
                    ];

                    for (item, detail) in gradle_items {
                        completions.push(CompletionItem {
                            label: item.to_string(),
                            kind: Some(CompletionItemKind::SNIPPET),
                            detail: Some(detail.to_string()),
                            insert_text: Some(item.to_string()),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        println!("Returning {} completion items", completions.len());
        completions
    }
}

















#[tower_lsp::async_trait]
impl LanguageServer for KotlinLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> JsonRpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    all_commit_characters: None,
                    work_done_progress_options: Default::default(),
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
        self.handle_completion_request(params).await
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        println!("Document opened: {}", params.text_document.uri);
        let uri = params.text_document.uri.to_string();
        let content = params.text_document.text;
        let mut documents = self.document_map.lock().await;
        documents.insert(uri.clone(), content);
        
        // Notify client that we received the document
        self.client
            .log_message(MessageType::INFO, format!("Document opened and indexed: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        println!("Document changed: {}", params.text_document.uri);
        if let Some(change) = params.content_changes.get(0) {
            let uri = params.text_document.uri.to_string();
            let mut documents = self.document_map.lock().await;
            documents.insert(uri.clone(), change.text.clone());
            
            // Notify client about the update
            self.client
                .log_message(MessageType::INFO, format!("Document updated: {}", uri))
                .await;
        }
    }
}
















pub struct LspClient {
    inner: Option<Client>,
}

impl LspClient {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn set_client(&mut self, client: Client) {
        self.inner = Some(client);
    }
}

pub struct LspManager {
    document_map: Arc<TokioMutex<HashMap<String, String>>>,
    completion_tx: mpsc::Sender<Vec<CompletionItem>>,
    completion_rx: mpsc::Receiver<Vec<CompletionItem>>,
    server: Option<tokio::task::JoinHandle<()>>,
    kotlin_server_process: Option<Child>,
    client: Arc<TokioMutex<LspClient>>,
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
            client: Arc::new(TokioMutex::new(LspClient::new())),
        }
    }

    fn detect_kotlin_home() -> Result<PathBuf, Box<dyn std::error::Error>> {
        // First try environment variable
        if let Ok(kotlin_home) = std::env::var("KOTLIN_HOME") {
            return Ok(PathBuf::from(kotlin_home));
        }

        // Try to find Android Studio installation
        let android_studio_path = if cfg!(windows) {
            vec![
                r"C:\Program Files\Android\Android Studio\plugins\Kotlin",
                r"C:\Program Files (x86)\Android\Android Studio\plugins\Kotlin",
            ]
        } else if cfg!(target_os = "macos") {
            vec![
                "/Applications/Android Studio.app/Contents/plugins/Kotlin",
                "~/Library/Application Support/Google/AndroidStudio*/plugins/Kotlin",
            ]
        } else {
            vec![
                "/usr/local/android-studio/plugins/Kotlin",
                "/opt/android-studio/plugins/Kotlin",
                "~/Android/android-studio/plugins/Kotlin",
            ]
        };

        // Try each potential path
        for path in android_studio_path {
            let expanded_path = shellexpand::tilde(path).into_owned();
            let path = Path::new(&expanded_path);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        // If no Kotlin installation found, try to use bundled server resources
        let fallback_path = Path::new("src/resources/kotlin");
        if fallback_path.exists() {
            return Ok(fallback_path.to_path_buf());
        }

        Err("Could not detect Kotlin installation. Please set KOTLIN_HOME environment variable.".into())
    }

    fn setup_kotlin_environment() -> Result<(), Box<dyn std::error::Error>> {
        let kotlin_home = Self::detect_kotlin_home()?;
        std::env::set_var("KOTLIN_HOME", kotlin_home.to_str().unwrap());
        
        // Set up additional environment variables if needed
        if std::env::var("JAVA_HOME").is_err() {
            // Try to detect Java installation
            let java_home = if cfg!(windows) {
                vec![
                    r"C:\Program Files\Java\jdk*",
                    r"C:\Program Files (x86)\Java\jdk*",
                ]
            } else if cfg!(target_os = "macos") {
                vec![
                    "/Library/Java/JavaVirtualMachines/*/Contents/Home",
                    "/System/Library/Java/JavaVirtualMachines/*/Contents/Home",
                ]
            } else {
                vec![
                    "/usr/lib/jvm/java-*",
                    "/usr/java/latest",
                ]
            };

            for pattern in java_home {
                if let Ok(paths) = glob::glob(&pattern) {
                    if let Some(path) = paths.filter_map(Result::ok).next() {
                        std::env::set_var("JAVA_HOME", path);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn setup_classpath_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Determine config directory based on platform
        let config_dir = if cfg!(windows) {
            let home = std::env::var("USERPROFILE")?;
            PathBuf::from(home).join(".config").join("kotlin-language-server")
        } else {
            let home = std::env::var("HOME")?;
            PathBuf::from(home).join(".config").join("kotlin-language-server")
        };

        // Create config directory if it doesn't exist
        fs::create_dir_all(&config_dir)?;

        // Create classpath script
        let script_path = if cfg!(windows) {
            config_dir.join("classpath.bat")
        } else {
            config_dir.join("classpath")
        };

        // Write script content
        let mut script_content = String::new();
        if cfg!(windows) {
            script_content.push_str("@echo off\n");
            script_content.push_str("echo %KOTLIN_HOME%\\lib\\kotlin-stdlib.jar;%KOTLIN_HOME%\\lib\\kotlin-compiler.jar");
        } else {
            script_content.push_str("#!/bin/bash\n");
            script_content.push_str("echo $KOTLIN_HOME/lib/kotlin-stdlib.jar:$KOTLIN_HOME/lib/kotlin-compiler.jar");
        }

        let mut file = fs::File::create(&script_path)?;
        file.write_all(script_content.as_bytes())?;

        // Make script executable on Unix-like systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }

        Ok(())
    }

    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting up Kotlin environment...");
        Self::setup_kotlin_environment()?;
        
        let kotlin_home = std::env::var("KOTLIN_HOME")
            .expect("KOTLIN_HOME should be set by setup_kotlin_environment");
        println!("Using KOTLIN_HOME: {}", kotlin_home);
        
        // Setup classpath configuration
        self.setup_classpath_config()?;
        
        let document_map = self.document_map.clone();
        let completion_tx = self.completion_tx.clone();
        let client = Arc::clone(&self.client);
        
        // Create a oneshot channel for passing the tower_lsp_client
        let (client_tx, client_rx) = tokio::sync::oneshot::channel();
        
        let (service, socket) = LspService::new(move |tower_lsp_client| {
            let _ = client_tx.send(tower_lsp_client.clone());
            
            KotlinLanguageServer::new(
                tower_lsp_client,
                document_map.clone(),
                completion_tx.clone(),
            )
        });
        
        // Set up the client
        let client_clone = client.clone();
        tokio::spawn(async move {
            if let Ok(tower_lsp_client) = client_rx.await {
                client_clone.lock().await.set_client(tower_lsp_client);
                println!("LSP client initialized successfully");
            }
        });
        
        // Start the LSP service
        self.server = Some(tokio::spawn(async move {
            println!("Starting LSP service...");
            tower_lsp::Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
                .serve(service)
                .await;
        }));

        println!("LSP server started successfully");
        Ok(())
    }

    async fn handle_completion_response(&self, response: CompletionResponse) {
        println!("Handling completion response");
        if let CompletionResponse::Array(items) = response {
            println!("Processing {} completion items", items.len());
            match self.completion_tx.send(items.clone()).await {
                Ok(_) => println!("Successfully sent completion items to UI"),
                Err(e) => eprintln!("Failed to send completion items: {}", e),
            }
        }
    }

    async fn get_kotlin_completions(&self, uri: &str, position: &Position) -> Result<Vec<CompletionItem>, Box<dyn std::error::Error>> {
        let mut completions = Vec::new();
        
        if let Some(_content) = self.document_map.lock().await.get(uri) {
            // Basic Kotlin keywords
            let keywords = vec![
                ("class", "Define a class"),
                ("fun", "Define a function"),
                ("val", "Immutable variable"),
                ("var", "Mutable variable"),
                ("if", "Conditional statement"),
                ("when", "Pattern matching"),
                ("for", "Loop construct"),
                ("while", "Loop construct"),
            ];
            
            for (keyword, detail) in keywords {
                completions.push(CompletionItem {
                    label: keyword.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(detail.to_string()),
                    insert_text: Some(keyword.to_string()),
                    documentation: Some(Documentation::String(format!("Kotlin {}", detail))),
                    ..Default::default()
                });
            }
        }

        Ok(completions)
    }
    
    pub async fn request_completions(&mut self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        println!("Requesting completions from LSP server...");
        
        // Ensure URI is properly formatted
        let uri = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };

        // Get the client from the shared state
        if let Some(client) = self.get_client().await {
            // Create completion params
            let params = CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: Url::parse(&uri)?,
                    },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
                context: None,
            };

            // Send the completion request using the notification channel
            println!("Sending completion request to LSP server");
            
            // Get completions from the Kotlin language server
            let completions = self.get_kotlin_completions(&uri, &position).await?;
            
            // Send completions through the channel
            if let Err(e) = self.completion_tx.send(completions).await {
                println!("Error sending completions through channel: {}", e);
                return Err(e.into());
            }
            
            Ok(())
        } else {
            Err("LSP client not initialized".into())
        }
    }

    async fn get_document_content(&self, uri: &str) -> Option<String> {
        self.document_map.lock().await.get(uri).cloned()
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

    async fn get_client(&self) -> Option<Client> {
        self.client.lock().await.inner.clone()
    }

    pub async fn update_document(&mut self, uri: String, content: String) {
        println!("Updating document: {}", uri);
        let uri = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };

        // Parse the URI string into a proper Url
        let uri_parsed = match Url::parse(&uri) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("Failed to parse URI {}: {}", uri, e);
                return;
            }
        };

        // Send didOpen notification if document is new
        let mut documents = self.document_map.lock().await;
        if !documents.contains_key(&uri) {
            if let Some(client) = self.get_client().await {
                let _params = DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri_parsed.clone(),
                        language_id: "kotlin".to_string(),
                        version: 1,
                        text: content.clone(),
                    },
                };
                println!("Sending didOpen notification for {}", uri);
                client.log_message(MessageType::INFO, format!("Opening document: {}", uri)).await;
            }
        } else {
            // Send didChange notification for existing document
            if let Some(client) = self.get_client().await {
                let _params = DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri_parsed,
                        version: documents.get(&uri).map(|_| 2).unwrap_or(1),
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: content.clone(),
                    }],
                };
                println!("Sending didChange notification for {}", uri);
                client.log_message(MessageType::INFO, format!("Updating document: {}", uri)).await;
            }
        }

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