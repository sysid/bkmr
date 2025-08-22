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
use tracing::{debug, info};

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
}

#[cfg(feature = "lsp")]
impl BkmrLspBackend {
    pub fn new(client: Client, config: BkmrConfig) -> Self {
        debug!("Creating BkmrLspBackend with config: {:?}", config);
        Self { client, config }
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

    async fn did_open(&self, _params: DidOpenTextDocumentParams) {
        debug!("Document opened");
    }

    async fn did_change(&self, _params: DidChangeTextDocumentParams) {
        debug!("Document changed");
    }

    async fn completion(&self, _params: CompletionParams) -> LspResult<Option<CompletionResponse>> {
        debug!("Completion requested");
        
        // For now, return empty completion list
        // This will be replaced with actual snippet completion in Phase 2
        Ok(Some(CompletionResponse::Array(vec![])))
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