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
use lsp_types::InitializedParams;
use tokio::process::Command as TokioCommand;

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

    async fn get_context_aware_completions(&self, content: &str, position: &Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        // Get the current line
        let lines: Vec<&str> = content.lines().collect();
        if let Some(line) = lines.get(position.line as usize) {
            let current_line = &line[..(position.character as usize).min(line.len())];
            
            // Use implemented completion functions based on context
            if current_line.ends_with('.') {
                completions.extend(self.get_kotlin_completions(
                    "file://temp.kt",
                    &Position {
                        line: position.line,
                        character: position.character,
                    },
                ).await);
            } else if current_line.contains("class") {
                completions.extend(vec![
                    CompletionItem {
                        label: "constructor".to_string(),
                        kind: Some(CompletionItemKind::CONSTRUCTOR),
                        detail: Some("Define a constructor".to_string()),
                        ..Default::default()
                    },
                    // Add more class-related completions
                ]);
            } else if current_line.contains("fun") {
                completions.extend(vec![
                    CompletionItem {
                        label: "private".to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("Private visibility modifier".to_string()),
                        ..Default::default()
                    },
                    // Add more function-related completions
                ]);
            } else {
                completions.extend(vec![
                    CompletionItem {
                        label: "class".to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("Define a class".to_string()),
                        ..Default::default()
                    },
                    // Add more basic completions
                ]);
            }
        }
        
        completions
    }

    async fn get_server_completions(&self, _uri: &str, _position: &Position) -> Option<Vec<CompletionItem>> {
        // Implement actual LSP server communication here
        // For now, returning None as placeholder
        None
    }

    async fn handle_completion_request(&self, params: CompletionParams) -> JsonRpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        // Get document content
        let document_content = {
            let documents = self.document_map.lock().await;
            documents.get(&uri).cloned()
        };

        if let Some(content) = document_content {
            // Get completions from multiple sources
            let mut completions = Vec::new();
            
            // Get context-aware completions
            let context_completions = self.get_context_aware_completions(&content, &position).await;
            completions.extend(context_completions);

            // Get LSP server completions
            if let Some(server_completions) = self.get_server_completions(&uri, &position).await {
                completions.extend(server_completions);
            }

            // Send completions through channel
            if let Err(e) = self.completion_tx.send(completions.clone()).await {
                eprintln!("Failed to send completions: {}", e);
            }

            Ok(Some(CompletionResponse::Array(completions)))
        } else {
            Ok(None)
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

    fn get_server_path(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        Ok(PathBuf::from("src")
            .join("resources")
            .join("server")
            .join("bin")
            .join("kotlin-language-server.bat"))
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
        let _client = Arc::clone(&self.client);
        
        // Create a oneshot channel for passing the tower_lsp_client
        let (client_tx, _client_rx) = tokio::sync::oneshot::channel();
        
        let (service, socket) = LspService::new(move |tower_lsp_client| {
            let _ = client_tx.send(tower_lsp_client.clone());
            KotlinLanguageServer::new(
                tower_lsp_client,
                document_map.clone(),
                completion_tx.clone(),
            )
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

    pub async fn get_completions(&mut self) -> Option<Vec<CompletionItem>> {
        match self.completion_rx.try_recv() {
            Ok(completions) => {
                println!("Received completions: {:?}", completions);
                Some(completions)
            }
            Err(_) => None,
        }
    }

    pub async fn connect_to_language_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let server_path = self.get_server_path()?;
        println!("Starting Kotlin Language Server from: {}", server_path.display());
    
        let process = TokioCommand::new(server_path)
            .env("JAVA_HOME", std::env::var("JAVA_HOME")?)
            .env("KOTLIN_HOME", std::env::var("KOTLIN_HOME")?)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
    
        self.kotlin_server_process = Some(process);
    
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; // Wait for server to start up
    
        if let Some(client) = self.get_client().await {
            client.log_message(MessageType::INFO, "Initializing LSP connection").await;
    
            // Initialize request
            let params = serde_json::to_value(InitializeParams {
                process_id: None,
                root_uri: None,
                capabilities: ClientCapabilities::default(),
                workspace_folders: None,
                client_info: Some(ClientInfo {
                    name: "ZZZ IDE".to_string(),
                    version: Some("0.1.0".to_string()),
                }),
                ..Default::default()
            })?;
            let params: InitializeParams = serde_json::from_value(params)?;
            client.send_request::<lsp_types::request::Initialize>(params).await?;
    
            // Send initialized notification
            client.send_notification::<lsp_types::notification::Initialized>(InitializedParams {}).await;
    
            // Optionally, you might want to set up some initial state or send additional notifications here
        }
    
        Ok(())
    }
    
    pub async fn update_document(&mut self, uri: String, content: String) -> Result<(), Box<dyn std::error::Error>> {
        println!("Updating document: {}", uri);
        let uri_str = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };
    
        let uri = match Url::parse(&uri_str) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("Failed to parse URI {}: {}", uri_str, e);
                return Ok(());
            }
        };
    
        let mut documents = self.document_map.lock().await;
        let version = documents.get(&uri_str).map(|_| 2).unwrap_or(1);
    
        if let Some(client) = self.get_client().await {
            documents.insert(uri_str.clone(), content.clone());
    
            if version == 1 {
                // Send DidOpenTextDocument notification
                let params = serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "kotlin".to_string(),
                        version: version as i32,
                        text: content.clone(),
                    },
                })?;
                let params: DidOpenTextDocumentParams = serde_json::from_value(params)?;
                client.send_notification::<lsp_types::notification::DidOpenTextDocument>(params).await;
            } else {
                // Send DidChangeTextDocument notification
                let params = serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: version as i32,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: content.clone(),
                    }],
                })?;
                let params: DidChangeTextDocumentParams = serde_json::from_value(params)?;
                client.send_notification::<lsp_types::notification::DidChangeTextDocument>(params).await;
            }
    
            // Handle completions
            if let Some(content) = documents.get(&uri_str) {
                let completions = self.generate_kotlin_completions(content).await;
                if let Err(e) = self.completion_tx.send(completions).await {
                    eprintln!("Failed to send completions: {}", e);
                }
            }
        }
    
        Ok(())
    }
    
    async fn get_client(&self) -> Option<Client> {
        self.client.lock().await.inner.clone()
    }
    
    async fn generate_kotlin_completions(&self, content: &str) -> Vec<CompletionItem> {
        let mut completions = Vec::new();
        
        // Basic Kotlin keywords and constructs
        let suggestions = vec![
            ("fun", "Function declaration", CompletionItemKind::KEYWORD),
            ("class", "Class declaration", CompletionItemKind::KEYWORD),
            ("val", "Immutable variable", CompletionItemKind::KEYWORD),
            ("var", "Mutable variable", CompletionItemKind::KEYWORD),
            ("if", "Conditional statement", CompletionItemKind::KEYWORD),
            ("when", "When expression", CompletionItemKind::KEYWORD),
            ("for", "For loop", CompletionItemKind::KEYWORD),
            ("while", "While loop", CompletionItemKind::KEYWORD),
            ("return", "Return statement", CompletionItemKind::KEYWORD),
            ("override", "Override modifier", CompletionItemKind::KEYWORD),
            ("private", "Private modifier", CompletionItemKind::KEYWORD),
            ("public", "Public modifier", CompletionItemKind::KEYWORD),
            ("protected", "Protected modifier", CompletionItemKind::KEYWORD),
            ("internal", "Internal modifier", CompletionItemKind::KEYWORD),
        ];
    
        // Context-aware completions based on content
        for (label, detail, kind) in suggestions {
            completions.push(CompletionItem {
                label: label.to_string(),
                kind: Some(kind),
                detail: Some(detail.to_string()),
                insert_text: Some(label.to_string()),
                documentation: Some(Documentation::String(detail.to_string())),
                ..Default::default()
            });
        }
    
        // Add Android-specific completions if we detect Android imports
        if content.contains("android.") {
            let android_completions = vec![
                ("Activity", "Android Activity class", CompletionItemKind::CLASS),
                ("Fragment", "Android Fragment class", CompletionItemKind::CLASS),
                ("Context", "Android Context class", CompletionItemKind::CLASS),
                ("View", "Android View class", CompletionItemKind::CLASS),
                ("onCreate", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
                ("onStart", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
                ("onResume", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
                ("onPause", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
                ("onStop", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
                ("onDestroy", "Activity/Fragment lifecycle method", CompletionItemKind::METHOD),
            ];
    
            for (label, detail, kind) in android_completions {
                completions.push(CompletionItem {
                    label: label.to_string(),
                    kind: Some(kind),
                    detail: Some(detail.to_string()),
                    insert_text: Some(label.to_string()),
                    documentation: Some(Documentation::String(detail.to_string())),
                    ..Default::default()
                });
            }
        }
    
        completions
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