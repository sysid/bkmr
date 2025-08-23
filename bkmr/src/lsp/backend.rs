//! LSP backend implementation for bkmr
//!
//! Provides Language Server Protocol functionality for snippet completion.

#[cfg(feature = "lsp")]
use tower_lsp::{Client, LanguageServer, LspService, Server};
#[cfg(feature = "lsp")]
use tower_lsp::jsonrpc::Result as LspResult;
#[cfg(feature = "lsp")]
use tower_lsp::lsp_types::*;
#[cfg(feature = "lsp")]
use tracing::{debug, error, info, instrument, warn};
#[cfg(feature = "lsp")]
use crate::domain::error::{DomainError, DomainResult};
#[cfg(feature = "lsp")]
use std::sync::Arc;
#[cfg(feature = "lsp")]
use std::collections::HashMap;

#[cfg(feature = "lsp")]
use crate::lsp::services::{CompletionService, LspSnippetService};
#[cfg(feature = "lsp")]
use crate::lsp::domain::{CompletionContext, CompletionQuery};

/// Configuration for the bkmr-lsp server
#[derive(Debug, Clone)]
pub struct BkmrConfig {
    pub bkmr_binary: String,
    pub max_completions: usize,
    pub enable_interpolation: bool,
}

impl Default for BkmrConfig {
    fn default() -> Self {
        Self {
            bkmr_binary: "bkmr".to_string(),
            max_completions: 50,
            enable_interpolation: true,
        }
    }
}

/// Main LSP backend structure
#[cfg(feature = "lsp")]
#[derive(Debug)]
pub struct BkmrLspBackend {
    client: Client,
    config: BkmrConfig,
    completion_service: CompletionService,
    /// Cache of document contents to extract prefixes
    document_cache: Arc<std::sync::RwLock<HashMap<String, String>>>,
    /// Cache of document language IDs for filetype-based filtering
    language_cache: Arc<std::sync::RwLock<HashMap<String, String>>>,
}

#[cfg(feature = "lsp")]
impl BkmrLspBackend {
    pub fn new(client: Client) -> Self {
        Self::with_config(client, BkmrConfig::default())
    }

    pub fn with_config(client: Client, config: BkmrConfig) -> Self {
        debug!("Creating BkmrLspBackend with config: {:?}", config);
        
        // Create snippet service 
        let snippet_service = Arc::new(LspSnippetService::new());
        
        // Create completion service with configuration
        let completion_service = CompletionService::with_config(snippet_service, config.clone());
        
        Self {
            client,
            config,
            completion_service,
            document_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            language_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Extract word backwards from cursor position and return both query and range
    #[instrument(skip(self))]
    fn extract_snippet_query(&self, uri: &Url, position: Position) -> Option<(String, Range)> {
        let cache = self.document_cache.read().ok()?;
        let content = cache.get(&uri.to_string())?;

        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let char_pos = position.character as usize;

        if char_pos > line.len() {
            return None;
        }

        let before_cursor = &line[..char_pos];
        debug!(
            "Extracting from line: '{}', char_pos: {}, before_cursor: '{}'",
            line, char_pos, before_cursor
        );

        // Extract word backwards from cursor - find where the word starts
        let word_start = before_cursor
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
            .last()
            .map(|(i, _)| i)
            .unwrap_or(char_pos);

        debug!("Word boundaries: start={}, end={}", word_start, char_pos);

        if word_start < char_pos {
            let word = &before_cursor[word_start..];
            if !word.is_empty() && word.chars().any(|c| c.is_alphanumeric()) {
                debug!("Extracted word: '{}' from position {}", word, char_pos);

                // Create range for the word to be replaced
                let range = Range {
                    start: Position {
                        line: position.line,
                        character: word_start as u32,
                    },
                    end: Position {
                        line: position.line,
                        character: char_pos as u32,
                    },
                };

                return Some((word.to_string(), range));
            }
        }

        debug!("No valid word found at position {}", char_pos);
        None
    }

    /// Get the language ID for a document URI
    fn get_language_id(&self, uri: &Url) -> Option<String> {
        let cache = self.language_cache.read().ok()?;
        cache.get(&uri.to_string()).cloned()
    }

    /// Check if bkmr binary is available
    #[instrument(skip(self))]
    async fn verify_bkmr_availability(&self) -> DomainResult<()> {
        debug!("Verifying bkmr availability");

        let command_future = tokio::process::Command::new(&self.config.bkmr_binary)
            .args(["--help"])
            .output();

        let output =
            match tokio::time::timeout(std::time::Duration::from_secs(5), command_future).await {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    return Err(DomainError::Other(format!("bkmr binary not found: {}", e)));
                }
                Err(_) => {
                    return Err(DomainError::Other("bkmr --help command timed out".to_string()));
                }
            };

        if !output.status.success() {
            return Err(DomainError::Other("bkmr binary is not working properly".to_string()));
        }

        info!("bkmr binary verified successfully");
        Ok(())
    }
}

#[cfg(feature = "lsp")]
#[tower_lsp::async_trait]
impl LanguageServer for BkmrLspBackend {
    #[instrument(skip(self, params))]
    async fn initialize(&self, params: InitializeParams) -> LspResult<InitializeResult> {
        info!(
            "Initialize request received from client: {:?}",
            params.client_info
        );

        // Verify bkmr is available
        if let Err(e) = self.verify_bkmr_availability().await {
            error!("bkmr verification failed: {}", e);
            self.client
                .log_message(
                    MessageType::ERROR,
                    &format!("Failed to verify bkmr availability: {}", e),
                )
                .await;
        }

        // Check if client supports snippets
        let snippet_support = params
            .capabilities
            .text_document
            .as_ref()
            .and_then(|td| td.completion.as_ref())
            .and_then(|comp| comp.completion_item.as_ref())
            .and_then(|item| item.snippet_support)
            .unwrap_or(false);

        info!("Client snippet support: {}", snippet_support);

        if !snippet_support {
            warn!("Client does not support snippets");
            self.client
                .log_message(
                    MessageType::WARNING,
                    "Client does not support snippets, functionality may be limited",
                )
                .await;
        }

        let result = InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None, // No automatic triggers - manual completion only
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                    completion_item: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        info!("Initialize complete - manual completion only (no trigger characters)");
        Ok(result)
    }

    #[instrument(skip(self))]
    async fn initialized(&self, _: InitializedParams) {
        info!("Server initialized successfully");

        self.client
            .log_message(MessageType::INFO, "bkmr-lsp server ready")
            .await;
    }

    #[instrument(skip(self))]
    async fn shutdown(&self) -> LspResult<()> {
        info!("Shutdown request received");
        self.client
            .log_message(MessageType::INFO, "Shutting down bkmr-lsp server")
            .await;
        Ok(())
    }

    #[instrument(skip(self, params))]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let content = params.text_document.text;
        let language_id = params.text_document.language_id;

        debug!("Document opened: {} (language: {})", uri, language_id);

        if let Ok(mut cache) = self.document_cache.write() {
            cache.insert(uri.clone(), content);
        }

        if let Ok(mut lang_cache) = self.language_cache.write() {
            lang_cache.insert(uri, language_id);
        }
    }

    #[instrument(skip(self, params))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        debug!("Document changed: {}", uri);

        if let Ok(mut cache) = self.document_cache.write() {
            for change in params.content_changes {
                if let Some(content) = cache.get_mut(&uri) {
                    // For FULL sync, replace entire content
                    if change.range.is_none() {
                        *content = change.text;
                    } else {
                        // For incremental sync, would need more complex logic
                        // For now, just replace entirely
                        *content = change.text;
                    }
                }
            }
        }
    }

    #[instrument(skip(self, params))]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        debug!("Document closed: {}", uri);

        if let Ok(mut cache) = self.document_cache.write() {
            cache.remove(&uri);
        }

        if let Ok(mut lang_cache) = self.language_cache.write() {
            lang_cache.remove(&uri);
        }
    }

    #[instrument(skip(self, params))]
    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        debug!(
            "Completion request for {}:{},{}",
            uri, position.line, position.character
        );

        // Only respond to manual completion requests (Ctrl+Space)
        if let Some(context) = &params.context {
            match context.trigger_kind {
                CompletionTriggerKind::INVOKED => {
                    // Manual Ctrl+Space - proceed with word-based completion
                    debug!("Manual completion request - proceeding with word-based snippet search");
                }
                CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS => {
                    debug!("Completion for incomplete results - proceeding");
                }
                _ => {
                    debug!("Ignoring automatic trigger - only manual completion supported");
                    return Ok(Some(CompletionResponse::Array(vec![])));
                }
            }
        } else {
            debug!("No completion context - skipping");
            return Ok(Some(CompletionResponse::Array(vec![])));
        }

        // Extract the query after trigger and get replacement range
        let query_info = self.extract_snippet_query(uri, position);
        debug!("Extracted snippet query info: {:?}", query_info);

        // Get the language ID for filetype-based filtering
        let language_id = self.get_language_id(uri);
        debug!("Document language ID: {:?}", language_id);

        // Create completion context for the service
        let mut context = CompletionContext::new(
            uri.clone(),
            position,
            language_id
        );
        
        // Add query information if extracted
        if let Some((query, range)) = query_info {
            debug!("Query: '{}', Range: {:?}", query, range);
            context = context.with_query(CompletionQuery::new(query, range));
        } else {
            debug!("No query extracted, using empty query");
        }

        // Use CompletionService to get completion items
        match self.completion_service.get_completions(&context).await {
            Ok(completion_items) => {
                info!(
                    "Returning {} completion items for query: {:?}",
                    completion_items.len(),
                    context.get_query_text().unwrap_or("")
                );

                // Only log first few items to reduce noise in LSP logs
                for (i, item) in completion_items.iter().enumerate().take(3) {
                    debug!(
                        "Item {}: label='{}', sort_text={:?}",
                        i, item.label, item.sort_text
                    );
                }
                if completion_items.len() > 3 {
                    debug!("... and {} more items", completion_items.len() - 3);
                }

                Ok(Some(CompletionResponse::List(CompletionList {
                    is_incomplete: true,
                    items: completion_items,
                })))
            }
            Err(e) => {
                error!("Failed to get completions: {}", e);
                self.client
                    .log_message(
                        MessageType::ERROR,
                        &format!("Failed to get completions: {}", e),
                    )
                    .await;
                Ok(Some(CompletionResponse::Array(vec![])))
            }
        }
    }
}

/// Run the LSP server
#[cfg(feature = "lsp")]
pub async fn run_server(no_interpolation: bool) {
    // Logging is now initialized in main.rs with proper color control
    // No need for duplicate initialization here
    
    // Get version from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("Starting bkmr LSP server v{}", version);
    info!("Starting bkmr LSP server v{}", version);

    // Create configuration
    let config = BkmrConfig {
        bkmr_binary: "bkmr".to_string(),
        max_completions: 50,
        enable_interpolation: !no_interpolation,
    };

    eprintln!("Configuration: {:?}", config);
    info!("Configuration: {:?}", config);

    // Validate environment before starting
    if let Err(e) = validate_environment().await {
        error!("Environment validation failed: {}", e);
        eprintln!("Environment validation failed: {}", e);
        std::process::exit(1);
    }

    // Set up the LSP service
    let (service, socket) = LspService::new(|client| BkmrLspBackend::with_config(client, config.clone()));

    eprintln!("LSP service created, starting server on stdin/stdout");
    info!("LSP service created, starting server on stdin/stdout");

    // Create server with stdin/stdout
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Start the server
    eprintln!("Starting LSP server loop");
    info!("Starting LSP server loop");
    Server::new(stdin, stdout, socket).serve(service).await;
    
    // If we reach here, the server has shut down gracefully
    info!("Server shutdown gracefully");
}

// Logging initialization has been moved to main.rs for centralization
// The main.rs setup_logging function now handles color control based on:
// 1. The global --no-color flag
// 2. Automatic detection of LSP command (forces no color for LSP)

/// Validate that the environment is suitable for running the LSP server
#[cfg(feature = "lsp")]
async fn validate_environment() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if we're in a proper LSP context (stdin/stdout should be available)
    if atty::is(atty::Stream::Stdin) || atty::is(atty::Stream::Stdout) {
        eprintln!("Warning: bkmr lsp is designed to run as an LSP server");
        eprintln!("It should be launched by an LSP client, not directly from a terminal");
        eprintln!("If you're testing, pipe some LSP messages to stdin");
    }

    // Test basic async functionality
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        tokio::task::yield_now().await
    })
    .await?;

    Ok(())
}

#[cfg(not(feature = "lsp"))]
pub async fn run_server(_no_interpolation: bool) {
    eprintln!("LSP support not compiled. Build with --features lsp");
    std::process::exit(1);
}