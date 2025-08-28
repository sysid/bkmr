// src/main.rs
use bkmr::infrastructure::di::ServiceContainer;
use bkmr::lsp::di::LspServiceContainer;
use bkmr::config::{load_settings, Settings};
use bkmr::cli::args::{Cli, Commands};
use bkmr::exitcode;
use clap::Parser;
use crossterm::style::Stylize;
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

    // Determine if colors should be disabled
    // Force no colors for LSP command to avoid ANSI escape sequences in LSP logs
    let no_color = cli.no_color || matches!(cli.command, Some(Commands::Lsp { .. }));

    setup_logging(cli.debug, no_color);

    // Load configuration with CLI overrides
    let config_path_ref = cli.config.as_deref();
    let settings = load_settings(config_path_ref)
        .unwrap_or_else(|e| {
            debug!("Failed to load settings: {}. Using defaults.", e);
            Settings::default()
        });
    
    // Note: OpenAI override from CLI flag will be handled in service container
    // when the embedder selection is properly implemented
    if cli.openai {
        debug!("OpenAI embeddings requested via CLI flag");
    }

    // Route to appropriate handler based on command
    match cli.command.as_ref() {
        Some(Commands::Lsp { no_interpolation }) => {
            if let Err(e) = handle_lsp_command(settings, *no_interpolation) {
                eprintln!("{}", format!("LSP error: {}", e).red());
                std::process::exit(exitcode::USAGE);
            }
        },
        _ => {
            // Create service container (single composition root)
            let service_container = match ServiceContainer::new(&settings) {
                Ok(container) => container,
                Err(e) => {
                    eprintln!("{}: {}", "Failed to create service container".red(), e);
                    std::process::exit(exitcode::USAGE);
                }
            };
            
            // Execute CLI command with services
            if let Err(e) = execute_command_with_services(stderr, cli, service_container, settings) {
                eprintln!("{}", format!("Error: {}", e).red());
                std::process::exit(exitcode::USAGE);
            }
        }
    }
}

fn handle_lsp_command(
    settings: Settings, 
    no_interpolation: bool
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::runtime::Runtime;

    // Create service containers for LSP
    let service_container = ServiceContainer::new(&settings)
        .map_err(|e| format!("Failed to create service container: {}", e))?;
    let _lsp_container = LspServiceContainer::new(&service_container, &settings);
    
    // Create a tokio runtime for the LSP server
    let rt = Runtime::new().map_err(|e| {
        format!("Failed to create async runtime: {}", e)
    })?;

    // Run the LSP server (for now, use existing implementation)
    rt.block_on(async {
        bkmr::lsp::run_lsp_server(&settings, no_interpolation).await;
    });

    Ok(())
}

fn execute_command_with_services(
    stderr: StandardStream,
    cli: Cli,
    services: ServiceContainer,
    settings: Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    bkmr::cli::execute_command_with_services(stderr, cli, services, &settings)
        .map_err(|e| format!("Command execution failed: {}", e).into())
}

fn setup_logging(verbosity: u8, no_color: bool) {
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
        .with_ansi(!no_color) // Control ANSI colors based on flag
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
    fn given_cli_command_when_verify_then_debug_asserts_pass() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
