// src/cli/mod.rs

use crate::cli::args::Commands;
use crate::context::{Context, CTX};
use crate::cli::error::{CliError, CliResult};
use args::Cli;
use clap::Parser;
use std::sync::RwLock;
use termcolor::{ColorChoice, StandardStream};
use tracing::level_filters::LevelFilter;
use tracing::{debug, info, instrument};
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, Layer};
use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::environment::CONFIG;
use crate::infrastructure::embeddings::{DummyEmbedding, OpenAiEmbedding};
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;

pub mod args;
pub mod commands;
pub mod error;
pub mod display;
pub mod process;
pub mod fzf;

#[instrument]
pub fn run() -> CliResult<()> {
    let cli = Cli::parse();
    let stderr = StandardStream::stderr(ColorChoice::Always);

    // Set up logging based on verbosity
    setup_logging(cli.debug);

    if let Some(Commands::CreateDb { .. }) = &cli.command {
        // Skip the path.exists check and create database with correct schema
    } else {
        let path = std::path::Path::new(&crate::environment::CONFIG.db_url);
        if !path.exists() {
            return Err(error::CliError::InvalidInput(format!(
                "Database path does not exist: {}",
                path.display()
            )));
        }

        commands::enable_embeddings_if_required()?;
    }

    // Set up context with appropriate embedding service
    let context = if cli.openai {
        Context::new(Box::new(OpenAiEmbedding::default()))
    } else {
        Context::new(Box::new(DummyEmbedding))
    };

    // Set the global context
    if CTX.set(RwLock::from(context)).is_err() {
        return Err(error::CliError::Other("Failed to initialize context".to_string()));
    }

    // Process command
    commands::execute_command(stderr, cli)
}

pub fn setup_logging(verbosity: u8) {
    debug!("INIT: Attempting logger init from cli/mod.rs");

    let filter = match verbosity {
        0 => LevelFilter::WARN,
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        3 => LevelFilter::TRACE,
        _ => {
            eprintln!("Don't be crazy, max is -d -d -d");
            LevelFilter::TRACE
        }
    };

    // Create a noisy module filter
    let noisy_modules = ["skim", "html5ever", "reqwest", "mio", "want", "tuikit"];
    let module_filter = filter_fn(move |metadata| {
        !noisy_modules
            .iter()
            .any(|name| metadata.target().starts_with(name))
    });

    // Create a subscriber with formatted output directed to stderr
    let fmt_layer = fmt::layer()
        .with_writer(std::io::stderr) // Set writer first
        .with_target(true)
        .with_thread_names(false)
        .with_span_events(FmtSpan::ENTER)
        .with_span_events(FmtSpan::CLOSE);

    // Apply filters to the layer
    let filtered_layer = fmt_layer.with_filter(filter).with_filter(module_filter);

    tracing_subscriber::registry().with(filtered_layer).init();

    // Log initial debug level
    match filter {
        LevelFilter::INFO => info!("Debug mode: info"),
        LevelFilter::DEBUG => debug!("Debug mode: debug"),
        LevelFilter::TRACE => debug!("Debug mode: trace"),
        _ => {}
    }
}

// Create a service factory function to reduce boilerplate
pub fn create_bookmark_service() -> CliResult<BookmarkApplicationService<SqliteBookmarkRepository>> {
    SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| CliError::RepositoryError(format!("Failed to create repository: {}", e)))
        .map(BookmarkApplicationService::new)
}


