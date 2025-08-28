//! LSP backend implementation for bkmr
//!
//! Provides Language Server Protocol functionality for snippet completion.

use crate::domain::error::{DomainError, DomainResult};
use serde_json::Value;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{debug, error, info, instrument, warn};

use crate::lsp::domain::{CompletionContext, CompletionQuery};
use crate::lsp::error::LspError;
use crate::lsp::services::{CommandService, CompletionService, DocumentService};

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
#[derive(Debug)]
pub struct BkmrLspBackend {
    client: Client,
    config: BkmrConfig,
    completion_service: CompletionService,
    document_service: DocumentService,
    command_service: CommandService,
}

impl BkmrLspBackend {

    /// Create backend with dependency injection (recommended)
    pub fn with_services(
        client: Client,
        config: BkmrConfig,
        completion_service: CompletionService,
        document_service: DocumentService,
        command_service: CommandService,
    ) -> Self {
        Self {
            client,
            config,
            completion_service,
            document_service,
            command_service,
        }
    }



    /// Extract word backwards from cursor position and return both query and range
    /// Delegates to DocumentService
    #[instrument(skip(self))]
    fn extract_snippet_query(&self, uri: &Url, position: Position) -> Option<(String, Range)> {
        self.document_service
            .extract_snippet_query_sync(uri, position)
    }

    /// Get the language ID for a document URI
    /// Delegates to DocumentService
    fn get_language_id(&self, uri: &Url) -> Option<String> {
        self.document_service.get_language_id_sync(uri)
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
                    return Err(DomainError::Other(
                        "bkmr --help command timed out".to_string(),
                    ));
                }
            };

        if !output.status.success() {
            return Err(DomainError::Other(
                "bkmr binary is not working properly".to_string(),
            ));
        }

        info!("bkmr binary verified successfully");
        Ok(())
    }
}

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
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "bkmr.insertFilepathComment".to_string(),
                        "bkmr.createSnippet".to_string(),
                        "bkmr.listSnippets".to_string(),
                        "bkmr.getSnippet".to_string(),
                        "bkmr.updateSnippet".to_string(),
                        "bkmr.deleteSnippet".to_string(),
                    ],
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "bkmr-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
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

        if let Err(e) = self
            .document_service
            .open_document(uri, language_id, content)
            .await
        {
            error!("Failed to open document: {}", e);
        }
    }

    #[instrument(skip(self, params))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        debug!("Document changed: {}", uri);

        for change in params.content_changes {
            // For FULL sync, replace entire content
            if change.range.is_none() {
                if let Err(e) = self
                    .document_service
                    .update_document(uri.clone(), change.text)
                    .await
                {
                    error!("Failed to update document: {}", e);
                }
            } else {
                // For incremental sync, would need more complex logic
                // For now, just replace entirely
                if let Err(e) = self
                    .document_service
                    .update_document(uri.clone(), change.text)
                    .await
                {
                    error!("Failed to update document: {}", e);
                }
            }
        }
    }

    #[instrument(skip(self, params))]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        debug!("Document closed: {}", uri);

        if let Err(e) = self.document_service.close_document(uri).await {
            error!("Failed to close document: {}", e);
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
        let mut context = CompletionContext::new(uri.clone(), position, language_id);

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

    #[instrument(skip(self, params))]
    async fn execute_command(&self, params: ExecuteCommandParams) -> LspResult<Option<Value>> {
        debug!("Execute command request: {}", params.command);

        match params.command.as_str() {
            "bkmr.insertFilepathComment" => {
                if let Some(arg) = params.arguments.first() {
                    if let Some(uri_str) = arg.as_str() {
                        debug!("Executing insertFilepathComment for URI: {}", uri_str);

                        match CommandService::insert_filepath_comment(uri_str) {
                            Ok(workspace_edit) => {
                                match self.client.apply_edit(workspace_edit).await {
                                    Ok(response) => {
                                        if response.applied {
                                            info!("Successfully applied filepath comment edit");
                                            return Ok(Some(serde_json::json!({"success": true})));
                                        } else {
                                            error!(
                                                "Client failed to apply edit: {:?}",
                                                response.failure_reason
                                            );
                                            return Ok(Some(serde_json::json!({
                                                "success": false,
                                                "error": response.failure_reason.unwrap_or_else(|| "Unknown error".to_string())
                                            })));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to send workspace edit to client: {}", e);
                                        return Ok(Some(serde_json::json!({
                                            "success": false,
                                            "error": format!("Failed to apply edit: {}", e)
                                        })));
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to create filepath comment edit: {}", e);
                                return Ok(Some(serde_json::json!({
                                    "success": false,
                                    "error": format!("Failed to create edit: {}", e)
                                })));
                            }
                        }
                    } else {
                        error!("Invalid argument format for insertFilepathComment");
                        return Ok(Some(serde_json::json!({
                            "success": false,
                            "error": "Invalid argument format"
                        })));
                    }
                } else {
                    error!("No arguments provided for insertFilepathComment command");
                    return Ok(Some(serde_json::json!({
                        "success": false,
                        "error": "No arguments provided"
                    })));
                }
            }
            "bkmr.createSnippet" => {
                // Parse arguments: {"url": "...", "title": "...", "description": "...", "tags": [...]}
                if let Some(arg) = params.arguments.first() {
                    let url = arg.get("url").and_then(|v| v.as_str());
                    let title = arg.get("title").and_then(|v| v.as_str());
                    let description = arg.get("description").and_then(|v| v.as_str());
                    let tags = arg
                        .get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    if let (Some(url), Some(title)) = (url, title) {
                        match self
                            .command_service
                            .create_snippet(url, title, description, tags)
                        {
                            Ok(result) => Ok(Some(result)),
                            Err(e) => Ok(Some(e.to_lsp_response())),
                        }
                    } else {
                        Ok(Some(
                            LspError::InvalidInput(
                                "Missing required fields: url and title".to_string(),
                            )
                            .to_lsp_response(),
                        ))
                    }
                } else {
                    Ok(Some(
                        LspError::InvalidInput("No arguments provided".to_string())
                            .to_lsp_response(),
                    ))
                }
            }
            "bkmr.listSnippets" => {
                // Parse optional language parameter
                let language_id = params
                    .arguments
                    .first()
                    .and_then(|arg| arg.get("language"))
                    .and_then(|v| v.as_str());

                match self.command_service.list_snippets(language_id) {
                    Ok(result) => Ok(Some(result)),
                    Err(e) => Ok(Some(e.to_lsp_response())),
                }
            }
            "bkmr.getSnippet" => {
                // Parse ID parameter
                if let Some(arg) = params.arguments.first() {
                    if let Some(id) = arg.get("id").and_then(|v| v.as_i64()) {
                        match self.command_service.get_snippet(id as i32) {
                            Ok(result) => Ok(Some(result)),
                            Err(e) => Ok(Some(e.to_lsp_response())),
                        }
                    } else {
                        Ok(Some(
                            LspError::InvalidInput("Missing or invalid id parameter".to_string())
                                .to_lsp_response(),
                        ))
                    }
                } else {
                    Ok(Some(
                        LspError::InvalidInput("No arguments provided".to_string())
                            .to_lsp_response(),
                    ))
                }
            }
            "bkmr.updateSnippet" => {
                // Parse update parameters
                if let Some(arg) = params.arguments.first() {
                    let id = arg.get("id").and_then(|v| v.as_i64()).map(|i| i as i32);
                    let url = arg.get("url").and_then(|v| v.as_str());
                    let title = arg.get("title").and_then(|v| v.as_str());
                    let description = arg.get("description").and_then(|v| v.as_str());
                    let tags = arg.get("tags").and_then(|v| v.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    });

                    if let Some(id) = id {
                        match self
                            .command_service
                            .update_snippet(id, url, title, description, tags)
                        {
                            Ok(result) => Ok(Some(result)),
                            Err(e) => Ok(Some(e.to_lsp_response())),
                        }
                    } else {
                        Ok(Some(
                            LspError::InvalidInput("Missing required field: id".to_string())
                                .to_lsp_response(),
                        ))
                    }
                } else {
                    Ok(Some(
                        LspError::InvalidInput("No arguments provided".to_string())
                            .to_lsp_response(),
                    ))
                }
            }
            "bkmr.deleteSnippet" => {
                // Parse ID parameter
                if let Some(arg) = params.arguments.first() {
                    if let Some(id) = arg.get("id").and_then(|v| v.as_i64()) {
                        match self.command_service.delete_snippet(id as i32) {
                            Ok(result) => Ok(Some(result)),
                            Err(e) => Ok(Some(e.to_lsp_response())),
                        }
                    } else {
                        Ok(Some(
                            LspError::InvalidInput("Missing or invalid id parameter".to_string())
                                .to_lsp_response(),
                        ))
                    }
                } else {
                    Ok(Some(
                        LspError::InvalidInput("No arguments provided".to_string())
                            .to_lsp_response(),
                    ))
                }
            }
            _ => {
                warn!("Unknown command: {}", params.command);
                Ok(Some(serde_json::json!({
                    "success": false,
                    "error": format!("Unknown command: {}", params.command)
                })))
            }
        }
    }
}

/// Run the LSP server
pub async fn run_server(settings: &crate::config::Settings, no_interpolation: bool) {
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

    // Create service containers with proper dependency injection
    use crate::infrastructure::di::ServiceContainer;
    
    let service_container = ServiceContainer::new(settings)
        .expect("Failed to create service container");

    // Set up the LSP service with proper dependency injection
    // Note: We need to recreate services inside the closure since they're not Clone
    let (service, socket) = LspService::new({
        let config = config.clone();
        let service_container = service_container;
        move |client| {
            use crate::lsp::services::{CompletionService, DocumentService, CommandService, LspSnippetService};
            
            // Create LSP services inside the closure
            let snippet_service = Arc::new(LspSnippetService::with_services(
                service_container.bookmark_service.clone(),
                service_container.template_service.clone(),
            ));
            
            let completion_service = CompletionService::new(snippet_service);
            let document_service = DocumentService::new();
            let command_service = CommandService::with_service(
                service_container.bookmark_service.clone()
            );
            
            BkmrLspBackend::with_services(
                client,
                config,
                completion_service,
                document_service,
                command_service,
            )
        }
    });

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
