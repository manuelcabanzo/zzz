use std::process::Stdio;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::process::{Child, Command};
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
    client: Arc<TokioMutex<LspClient>>, // Add this field
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

    // Modify start_server to use the classpath config
    pub async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Setting up Kotlin environment...");
        Self::setup_kotlin_environment()?;
    
        let kotlin_home = std::env::var("KOTLIN_HOME")
            .expect("KOTLIN_HOME should be set by setup_kotlin_environment");
        println!("Using KOTLIN_HOME: {}", kotlin_home);
    
        // Setup classpath configuration
        self.setup_classpath_config()?;
    
        let server_path = Path::new("src/resources/server/bin/kotlin-language-server");
        if !server_path.exists() {
            return Err("Kotlin LSP server not found at expected path".into());
        }
    
        // Clone the values we need before moving them into the closure
        let document_map = self.document_map.clone();
        let completion_tx = self.completion_tx.clone();
        let client = Arc::clone(&self.client);
    
        let (_service, _socket) = LspService::new(move |tower_lsp_client| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                client.lock().await.set_client(tower_lsp_client.clone());
            });
            
            KotlinLanguageServer::new(
                tower_lsp_client,
                document_map.clone(),
                completion_tx.clone(),
            )
        });
    
        // Start the server with proper environment setup
        let mut command = Command::new(server_path);
        command
            .env("KOTLIN_HOME", &kotlin_home)
            .env("PATH", format!("{};{}", 
                kotlin_home.clone(), 
                std::env::var("PATH").unwrap_or_default()
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
    
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            command.env("JAVA_HOME", java_home);
        }
    
        println!("Starting Kotlin LSP server...");
        self.kotlin_server_process = Some(command.spawn()?);
        
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

    pub async fn request_completions(&mut self, uri: String, position: Position) -> Result<(), Box<dyn std::error::Error>> {
        let uri = if !uri.starts_with("file://") {
            format!("file://{}", uri.replace('\\', "/"))
        } else {
            uri
        };

        let uri_parsed = Url::parse(&uri)?;

        let _params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: uri_parsed,
                },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: Some(NumberOrString::String("completion-token".to_string())),
            },
            partial_result_params: PartialResultParams {
                partial_result_token: Some(NumberOrString::String("partial-token".to_string())),
            },
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };

        println!("Preparing to send completion request...");
        
        // Add a timeout for completion requests
        let completion_future = async {
            if let Some(response) = self.get_completions() {
                println!("Received completion response with {} items", response.len());
                self.handle_completion_response(CompletionResponse::Array(response)).await;
                Ok(())
            } else {
                println!("No completion response received");
                Err("No completion response".into())
            }
        };

        match tokio::time::timeout(std::time::Duration::from_secs(5), completion_future).await {
            Ok(result) => {
                println!("Completion request completed: {:?}", result);
                result
            },
            Err(_) => {
                println!("Completion request timed out");
                Err("Completion request timed out".into())
            }
        }
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