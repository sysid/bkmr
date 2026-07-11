//! LSP backend implementation for bkmr
//!
//! Provides Language Server Protocol functionality for snippet completion.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{debug, error, info, instrument, warn};

use crate::lsp::error::LspError;
use crate::lsp::services::{CommandService, CompletionService, DocumentService};

/// Configuration for the bkmr-lsp server
#[derive(Debug, Clone)]
pub struct BkmrConfig {
    pub max_completions: usize,
    pub enable_interpolation: bool,
}

impl Default for BkmrConfig {
    fn default() -> Self {
        Self {
            max_completions: 50,
            enable_interpolation: true,
        }
    }
}

/// Main LSP backend structure
#[derive(Debug)]
pub struct BkmrLspBackend {
    client: Client,
    completion_service: CompletionService,
    document_service: DocumentService,
    command_service: CommandService,
}

/// Arguments for the bkmr.createSnippet command
#[derive(Deserialize)]
struct CreateSnippetArgs {
    url: String,
    title: String,
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Arguments for commands addressing a snippet by ID
#[derive(Deserialize)]
struct SnippetIdArgs {
    id: i32,
}

/// Arguments for the bkmr.updateSnippet command
#[derive(Deserialize)]
struct UpdateSnippetArgs {
    id: i32,
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
}

/// Parse the first execute_command argument into a typed struct
fn parse_command_args<T: DeserializeOwned>(params: &ExecuteCommandParams) -> Result<T, LspError> {
    let arg = params
        .arguments
        .first()
        .ok_or_else(|| LspError::InvalidInput("No arguments provided".to_string()))?;
    serde_json::from_value(arg.clone())
        .map_err(|e| LspError::InvalidInput(format!("Invalid arguments: {}", e)))
}

impl BkmrLspBackend {
    /// Create backend with dependency injection
    pub fn with_services(
        client: Client,
        completion_service: CompletionService,
        document_service: DocumentService,
        command_service: CommandService,
    ) -> Self {
        Self {
            client,
            completion_service,
            document_service,
            command_service,
        }
    }

    /// Execute the insertFilepathComment command by applying a workspace edit
    async fn handle_insert_filepath_comment(&self, params: &ExecuteCommandParams) -> Value {
        let Some(uri_str) = params.arguments.first().and_then(|arg| arg.as_str()) else {
            error!("Missing or invalid URI argument for insertFilepathComment");
            return serde_json::json!({
                "success": false,
                "error": "Missing or invalid URI argument"
            });
        };

        debug!("Executing insertFilepathComment for URI: {}", uri_str);

        let workspace_edit = match CommandService::insert_filepath_comment(uri_str) {
            Ok(edit) => edit,
            Err(e) => {
                error!("Failed to create filepath comment edit: {}", e);
                return serde_json::json!({
                    "success": false,
                    "error": format!("Failed to create edit: {}", e)
                });
            }
        };

        match self.client.apply_edit(workspace_edit).await {
            Ok(response) if response.applied => {
                info!("Successfully applied filepath comment edit");
                serde_json::json!({"success": true})
            }
            Ok(response) => {
                error!("Client failed to apply edit: {:?}", response.failure_reason);
                serde_json::json!({
                    "success": false,
                    "error": response.failure_reason.unwrap_or_else(|| "Unknown error".to_string())
                })
            }
            Err(e) => {
                error!("Failed to send workspace edit to client: {}", e);
                serde_json::json!({
                    "success": false,
                    "error": format!("Failed to apply edit: {}", e)
                })
            }
        }
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
        debug!(
            "Document opened: {} (language: {})",
            uri, params.text_document.language_id
        );

        self.document_service
            .open_document(
                uri,
                params.text_document.language_id,
                params.text_document.text,
            )
            .await;
    }

    #[instrument(skip(self, params))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        debug!("Document changed: {}", uri);

        // Sync kind is FULL, so every change carries the complete document text
        for change in params.content_changes {
            self.document_service
                .update_document(uri.clone(), change.text)
                .await;
        }
    }

    #[instrument(skip(self, params))]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        debug!("Document closed: {}", uri);
        self.document_service.close_document(uri).await;
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
        match params.context.as_ref().map(|c| c.trigger_kind) {
            Some(CompletionTriggerKind::INVOKED) => {
                debug!("Manual completion request - proceeding with word-based snippet search");
            }
            Some(CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS) => {
                debug!("Completion for incomplete results - proceeding");
            }
            _ => {
                debug!("Ignoring automatic trigger - only manual completion supported");
                return Ok(Some(CompletionResponse::Array(vec![])));
            }
        }

        let context = self
            .document_service
            .extract_completion_context(uri, position)
            .await;
        debug!(
            "Completion context: query={:?}, language={:?}",
            context.get_query_text(),
            context.language_id
        );

        match self.completion_service.get_completions(&context).await {
            Ok(completion_items) => {
                info!(
                    "Returning {} completion items for query: {:?}",
                    completion_items.len(),
                    context.get_query_text().unwrap_or("")
                );

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

        let result = match params.command.as_str() {
            "bkmr.insertFilepathComment" => {
                return Ok(Some(self.handle_insert_filepath_comment(&params).await));
            }
            "bkmr.createSnippet" => {
                parse_command_args::<CreateSnippetArgs>(&params).and_then(|args| {
                    self.command_service.create_snippet(
                        &args.url,
                        &args.title,
                        args.description.as_deref(),
                        args.tags,
                    )
                })
            }
            "bkmr.listSnippets" => {
                // The language argument is optional, as is the argument object itself
                let language = params
                    .arguments
                    .first()
                    .and_then(|arg| arg.get("language"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                self.command_service.list_snippets(language.as_deref())
            }
            "bkmr.getSnippet" => parse_command_args::<SnippetIdArgs>(&params)
                .and_then(|args| self.command_service.get_snippet(args.id)),
            "bkmr.updateSnippet" => {
                parse_command_args::<UpdateSnippetArgs>(&params).and_then(|args| {
                    self.command_service.update_snippet(
                        args.id,
                        args.url.as_deref(),
                        args.title.as_deref(),
                        args.description.as_deref(),
                        args.tags,
                    )
                })
            }
            "bkmr.deleteSnippet" => parse_command_args::<SnippetIdArgs>(&params)
                .and_then(|args| self.command_service.delete_snippet(args.id)),
            _ => {
                warn!("Unknown command: {}", params.command);
                return Ok(Some(serde_json::json!({
                    "success": false,
                    "error": format!("Unknown command: {}", params.command)
                })));
            }
        };

        match result {
            Ok(value) => Ok(Some(value)),
            Err(e) => Ok(Some(e.to_lsp_response())),
        }
    }
}

/// Run the LSP server
pub async fn run_server(settings: &crate::config::Settings, no_interpolation: bool) {
    // Logging is initialized in main.rs with proper color control

    let version = env!("CARGO_PKG_VERSION");
    info!("Starting bkmr LSP server v{}", version);

    let config = BkmrConfig {
        max_completions: 50,
        enable_interpolation: !no_interpolation,
    };

    info!("Configuration: {:?}", config);

    // Validate environment before starting
    if let Err(e) = validate_environment().await {
        error!("Environment validation failed: {}", e);
        std::process::exit(1);
    }

    use crate::infrastructure::di::ServiceContainer;
    use crate::lsp::di::LspServiceContainer;

    let service_container =
        ServiceContainer::new(settings).expect("Failed to create service container");

    let (service, socket) = LspService::new(move |client| {
        let lsp_services = LspServiceContainer::new(
            service_container.bookmark_service.clone(),
            service_container.interpolation_service.clone(),
            config.clone(),
        );

        BkmrLspBackend::with_services(
            client,
            lsp_services.completion_service,
            lsp_services.document_service,
            lsp_services.command_service,
        )
    });

    info!("LSP service created, starting server on stdin/stdout");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    info!("Starting LSP server loop");
    Server::new(stdin, stdout, socket).serve(service).await;

    info!("Server shutdown gracefully");
}

/// Validate that the environment is suitable for running the LSP server
async fn validate_environment() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if we're in a proper LSP context (stdin/stdout should be available)
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() || std::io::stdout().is_terminal() {
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
