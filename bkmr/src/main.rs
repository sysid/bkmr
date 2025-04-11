// src/main.rs
use bkmr::infrastructure::embeddings::{DummyEmbedding, OpenAiEmbedding};

use bkmr::app_state::AppState;
use bkmr::cli::args::Cli;
use bkmr::cli::execute_command;
use bkmr::domain::embedding::Embedder;
use clap::Parser;
use crossterm::style::Stylize;
use std::sync::Arc;
use termcolor::{ColorChoice, StandardStream};
use tracing::{debug, info, instrument};
use tracing_subscriber::{
    filter::{filter_fn, LevelFilter},
    fmt::{self, format::FmtSpan},
    prelude::*,
};

#[instrument]
fn main() {
    // use stderr as human output in order to make stdout output passable to downstream processes
    let stderr = StandardStream::stderr(ColorChoice::Always);
    let cli = Cli::parse();
    setup_logging(cli.debug);

    // Create embedder based on CLI option
    let embedder: Arc<dyn Embedder> = if cli.openai {
        debug!("OpenAI embeddings enabled");
        Arc::new(OpenAiEmbedding::default())
    } else {
        debug!("Using DummyEmbedding (no embeddings will be stored)");
        Arc::new(DummyEmbedding)
    };

    // Convert config_file to Path reference if provided
    let config_path_ref = cli.config_file.as_deref();

    // Initialize AppState with the embedder and config file
    let app_state = AppState::new_with_config_file(embedder, config_path_ref);
    let result = AppState::update_global(app_state);

    if let Err(e) = result {
        eprintln!("{}: {}", "Failed to initialize AppState".red(), e);
        std::process::exit(1);
    }

    // Execute the command
    if let Err(e) = execute_command(stderr, cli) {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    }
}

fn setup_logging(verbosity: u8) {
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
    let noisy_modules = [
        "skim",
        "html5ever",
        "reqwest",
        "mio",
        "want",
        "tuikit",
        "hyper_util",
    ];
    let module_filter = filter_fn(move |metadata| {
        !noisy_modules
            .iter()
            .any(|name| metadata.target().starts_with(name))
    });

    // Create a subscriber with formatted output directed to stderr
    let fmt_layer = fmt::layer()
        .with_writer(std::io::stderr) // Set writer first
        .with_target(true)
        // src/main.rs (continued)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
