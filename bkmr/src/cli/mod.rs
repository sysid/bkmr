// src/cli/mod.rs
pub mod args;
pub mod commands;
pub mod error;

use crate::adapter::embeddings::{DummyEmbedding, OpenAiEmbedding};
use crate::context::{Context, CTX};
use anyhow::Result;
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
use crate::cli::args::Commands;
use crate::environment::CONFIG;

#[instrument]
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    // let stdout = StandardStream::stdout(ColorChoice::Always);
    // use stderr as human output in order to make stdout output passable to downstream processes
    let stderr = StandardStream::stderr(ColorChoice::Always);

    // Set up logging based on verbosity
    setup_logging(cli.debug);

    if let Some(Commands::CreateDb { .. }) = &cli.command {
        // Skip the path.exists check and create database with correct schema
    } else {
        todo!();
        // let path = std::path::Path::new(&CONFIG.db_url);
        // if !path.exists() {
        //     eprintln!("Error: db_url path does not exist: {:?}", CONFIG.db_url);
        //     std::process::exit(1);
        // }
        // commands::enable_embeddings_if_required().expect("Failed to enable embeddings");
    }

    // Set up context with appropriate embedding service
    let context = if cli.openai {
        Context::new(Box::new(OpenAiEmbedding::default()))
    } else {
        Context::new(Box::new(DummyEmbedding))
    };

    // Set the global context
    if CTX.set(RwLock::from(context)).is_err() {
        eprintln!("Failed to initialize context");
        std::process::exit(1);
    }

    // Process command
    if let Err(e) = commands::execute_command(stderr, cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
pub fn setup_logging(verbosity: u8) {
    debug!("INIT: Attempting logger init from main.rs");

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
