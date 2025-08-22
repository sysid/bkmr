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
use tracing::{debug, info, instrument};

#[cfg(feature = "lsp")]
use crate::lsp::services::{AsyncSnippetService, LspSnippetService};
#[cfg(feature = "lsp")]
use crate::lsp::domain::SnippetFilter;
#[cfg(feature = "lsp")]
use std::collections::HashMap;

/// Configuration for the bkmr-lsp server
#[derive(Debug, Clone)]
pub struct BkmrConfig {
    pub enable_interpolation: bool,
}

impl Default for BkmrConfig {
    fn default() -> Self {
        Self {
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
    snippet_service: LspSnippetService,
    /// Cache of document contents to extract prefixes
    document_cache: std::sync::Arc<std::sync::RwLock<HashMap<String, String>>>,
    /// Cache of document language IDs for filetype-based filtering
    language_cache: std::sync::Arc<std::sync::RwLock<HashMap<String, String>>>,
}

#[cfg(feature = "lsp")]
impl BkmrLspBackend {
    pub fn new(client: Client, config: BkmrConfig) -> Self {
        debug!("Creating BkmrLspBackend with config: {:?}", config);
        Self { 
            client, 
            config,
            snippet_service: LspSnippetService::new(),
            document_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
            language_cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
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
}

#[cfg(feature = "lsp")]
#[tower_lsp::async_trait]
impl LanguageServer for BkmrLspBackend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        info!("LSP server initialized");
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![":".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("LSP server ready");
        self.client
            .log_message(MessageType::INFO, "bkmr LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        info!("LSP server shutting down");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        debug!("Document opened: {}", params.text_document.uri);
        
        // Cache the document content
        if let Ok(mut cache) = self.document_cache.write() {
            cache.insert(params.text_document.uri.to_string(), params.text_document.text);
        }
        
        // Cache the language ID
        if let Ok(mut cache) = self.language_cache.write() {
            cache.insert(params.text_document.uri.to_string(), params.text_document.language_id);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("Document changed: {}", params.text_document.uri);
        
        // Update the document cache with the new content
        if let Some(change) = params.content_changes.into_iter().next() {
            if let Ok(mut cache) = self.document_cache.write() {
                cache.insert(params.text_document.uri.to_string(), change.text);
            }
        }
    }

    #[instrument(skip(self))]
    async fn completion(&self, params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        debug!("Completion requested");
        
        let position = params.text_document_position.position;
        let uri = &params.text_document_position.text_document.uri;
        
        // Extract the query from the cursor position
        let (query_text, range) = match self.extract_snippet_query(uri, position) {
            Some(result) => result,
            None => {
                debug!("No query found at cursor position");
                return Ok(Some(CompletionResponse::Array(vec![])));
            }
        };
        
        debug!("Extracted query: '{}' at range: {:?}", query_text, range);
        
        // Get the language ID for filtering
        let language_id = self.get_language_id(uri);
        debug!("Language ID: {:?}", language_id);
        
        // Create snippet filter
        let filter = SnippetFilter::new(
            language_id,
            Some(query_text.clone()),
            50, // Max results
        );
        
        // Fetch snippets using the service
        let snippets = match self.snippet_service.fetch_snippets(&filter).await {
            Ok(snippets) => snippets,
            Err(e) => {
                debug!("Error fetching snippets: {}", e);
                return Ok(Some(CompletionResponse::Array(vec![])));
            }
        };
        
        debug!("Found {} snippets", snippets.len());
        
        // Convert snippets to LSP completion items
        let completion_items: Vec<CompletionItem> = snippets
            .into_iter()
            .map(|snippet| {
                CompletionItem {
                    label: snippet.title.clone(),
                    kind: Some(CompletionItemKind::SNIPPET),
                    detail: Some(snippet.description.clone()),
                    insert_text: Some(snippet.content.clone()),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: snippet.content,
                    })),
                    ..Default::default()
                }
            })
            .collect();
        
        debug!("Returning {} completion items", completion_items.len());
        Ok(Some(CompletionResponse::Array(completion_items)))
    }
}

/// Run the LSP server
#[cfg(feature = "lsp")]
pub async fn run_server(no_interpolation: bool) {
    // Initialize logging
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    info!("Starting bkmr LSP server");

    // Create configuration
    let config = BkmrConfig {
        enable_interpolation: !no_interpolation,
    };

    // Set up the LSP service
    let (service, socket) = LspService::new(|client| BkmrLspBackend::new(client, config.clone()));

    // Create server with stdin/stdout
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Start the server
    info!("LSP server starting on stdin/stdout");
    Server::new(stdin, stdout, socket).serve(service).await;
    info!("LSP server shutdown complete");
}

#[cfg(feature = "lsp")]
fn init_logging() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("bkmr=info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false) // Disable color codes for LSP compatibility
        .with_target(false) // Reduce noise in LSP logs
        .with_env_filter(filter)
        .try_init()?;

    Ok(())
}

#[cfg(not(feature = "lsp"))]
pub async fn run_server(_no_interpolation: bool) {
    eprintln!("LSP support not compiled. Build with --features lsp");
    std::process::exit(1);
}