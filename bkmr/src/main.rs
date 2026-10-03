// src/main.rs
use bkmr::cli::args::{Cli, Commands};
use bkmr::cli::bookmark_commands::pre_fill_database;
use bkmr::config::{load_settings, ConfigSource, Settings};
use bkmr::exitcode;
use bkmr::infrastructure::di::ServiceContainer;
use bkmr::infrastructure::repositories::sqlite::{migration, repository::SqliteBookmarkRepository};
use bkmr::util::helper::confirm;
use clap::Parser;
use crossterm::style::Stylize;
use std::fs;
use std::path::Path;
use tracing::{debug, info, instrument};
use tracing_subscriber::{
    filter::{filter_fn, LevelFilter},
    fmt::{self, format::FmtSpan},
    prelude::*,
};

#[instrument]
fn main() {
    // Register sqlite-vec before any database connections
    bkmr::infrastructure::repositories::sqlite::register_sqlite_vec();

    let cli = Cli::parse();

    // Determine if colors should be disabled.
    // Honor the --no-color flag and the NO_COLOR convention (https://no-color.org/).
    let no_color_requested = cli.no_color || std::env::var_os("NO_COLOR").is_some();
    // Force no colors for LSP command to avoid ANSI escape sequences in LSP logs.
    let no_color = no_color_requested || matches!(cli.command, Some(Commands::Lsp { .. }));

    setup_logging(cli.debug, no_color);

    // Load configuration with CLI overrides
    let config_path_ref = cli.config.as_deref();
    let mut settings = load_settings(config_path_ref).unwrap_or_else(|e| {
        debug!("Failed to load settings: {}. Using defaults.", e);
        Settings::default()
    });

    // CLI --db flag takes highest priority
    if let Some(ref db_path) = cli.db {
        settings.db_url = db_path.to_string_lossy().to_string();
    }

    // Propagate the resolved colour opt-out so display code can honor it
    // (display writes to a TTY; the LSP-forced value is irrelevant there).
    settings.no_color = no_color_requested;

    info!(db_url = %settings.db_url, config_source = ?settings.config_source, "Configuration loaded");

    // Handle all database-independent operations first
    if let Some(result) = handle_database_independent_operations(cli.clone(), &settings) {
        if let Err(e) = result {
            eprintln!("{}", format!("Error: {}", e).red());
            std::process::exit(exitcode::USAGE);
        }
        return;
    }

    // Only create ServiceContainer for database-dependent operations
    let service_container = match ServiceContainer::new(&settings) {
        Ok(container) => container,
        Err(e) => {
            eprintln!("{}: {}", "Failed to create service container".red(), e);
            std::process::exit(exitcode::USAGE);
        }
    };

    // Execute CLI command with services
    if let Err(e) = execute_command_with_services(cli, service_container, settings) {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(exitcode::USAGE);
    }
}

fn handle_create_db_command(
    cli: Cli,
    settings: &Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Commands::CreateDb { path, pre_fill } = cli.command.unwrap() {
        // Get the database path using existing precedence: CLI argument -> config -> default
        let db_path = match path {
            Some(p) => p,
            None => {
                // Get from config system via settings parameter
                let configured_path = &settings.db_url;

                // Check if we're using default configuration
                if settings.config_source == ConfigSource::Default {
                    eprintln!(
                        "{}",
                        "Warning: Using default database path. No configuration found.".yellow()
                    );
                    eprintln!("Default path: {}", configured_path);
                    eprintln!(
                        "Consider creating a configuration file at ~/.config/bkmr/config.toml"
                    );
                    eprintln!("or setting the BKMR_DB_URL environment variable.");

                    // Ask for confirmation when using default configuration
                    if !confirm("Continue with default database location?") {
                        eprintln!("Database creation cancelled.");
                        return Ok(());
                    }
                }

                configured_path.clone()
            }
        };

        // Check if the database file already exists
        if Path::new(&db_path).exists() {
            return Err(format!(
                "Database already exists at: {}. Please choose a different path or delete the existing file.",
                db_path
            ).into());
        }

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&db_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directories: {}", e))?;
            }
        }

        eprintln!("Creating new database at: {}", db_path);

        // Create the repository with the new path
        let repository = SqliteBookmarkRepository::from_url(&db_path)
            .map_err(|e| format!("Failed to create repository: {}", e))?;

        // Get a connection
        let mut conn = repository
            .get_connection()
            .map_err(|e| format!("Failed to get database connection: {}", e))?;

        // Run migrations to set up the schema
        migration::init_db(&mut conn)
            .map_err(|e| format!("Failed to initialize database: {}", e))?;

        eprintln!("Database created successfully at: {}", db_path);

        // Handle pre-fill if requested
        if pre_fill {
            eprintln!("Pre-filling database with demo entries...");
            pre_fill_database(&repository)
                .map_err(|e| format!("Failed to pre-fill database: {}", e))?;
            eprintln!("Database pre-filled with demo entries.");
        }
    }
    Ok(())
}

/// Handle all database-independent operations (flags and commands that don't need ServiceContainer)
fn handle_database_independent_operations(
    cli: Cli,
    settings: &Settings,
) -> Option<Result<(), Box<dyn std::error::Error>>> {
    // Handle flags first
    if cli.generate_config {
        return Some(handle_generate_config());
    }

    // Handle commands that don't need database
    match cli.command.as_ref() {
        Some(Commands::CreateDb { .. }) => Some(handle_create_db_command(cli, settings)),
        Some(Commands::Completion { shell }) => Some(handle_completion_command(shell.clone())),
        _ => None, // Requires database services
    }
}

fn handle_generate_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", bkmr::config::generate_default_config());
    Ok(())
}

fn handle_completion_command(shell: String) -> Result<(), Box<dyn std::error::Error>> {
    // Write a brief comment to stderr about what's being output
    match shell.to_lowercase().as_str() {
        "bash" => {
            eprintln!("# Outputting bash completion script for bkmr");
            eprintln!("# To use, run one of:");
            eprintln!("# - eval \"$(bkmr completion bash)\"                   # one-time use");
            eprintln!("# - bkmr completion bash >> ~/.bashrc                  # add to bashrc");
            eprintln!(
                "# - bkmr completion bash > /etc/bash_completion.d/bkmr # system-wide install"
            );
            eprintln!("#");
        }
        "zsh" => {
            eprintln!("# Outputting zsh completion script for bkmr");
            eprintln!("# To use, run one of:");
            eprintln!("# - eval \"$(bkmr completion zsh)\"                    # one-time use");
            eprintln!(
                "# - bkmr completion zsh > ~/.zfunc/_bkmr               # save to fpath directory"
            );
            eprintln!("# - echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc         # add dir to fpath if needed");
            eprintln!("# - echo 'autoload -U compinit && compinit' >> ~/.zshrc # load completions");
            eprintln!("#");
        }
        "fish" => {
            eprintln!("# Outputting fish completion script for bkmr");
            eprintln!("# To use, run one of:");
            eprintln!("# - bkmr completion fish | source                      # one-time use");
            eprintln!("# - bkmr completion fish > ~/.config/fish/completions/bkmr.fish # permanent install");
            eprintln!("#");
        }
        _ => {}
    }

    // Generate completion script to stdout
    match bkmr::cli::completion::generate_completion(&shell) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to generate completion script: {}", e).into()),
    }
}

#[derive(Debug)]
struct CommandExecutionError(bkmr::cli::error::CliError);

impl std::fmt::Display for CommandExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Command execution failed: {}", self.0)
    }
}

impl std::error::Error for CommandExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

fn command_execution_error(error: bkmr::cli::error::CliError) -> Box<dyn std::error::Error> {
    Box::new(CommandExecutionError(error))
}

fn execute_command_with_services(
    cli: Cli,
    services: ServiceContainer,
    settings: Settings,
) -> Result<(), Box<dyn std::error::Error>> {
    bkmr::cli::execute_command_with_services(cli, services, &settings)
        .map_err(command_execution_error)
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
    let noisy_modules = ["skim", "html5ever", "reqwest", "mio", "want", "hyper_util"];
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

    #[test]
    fn given_actual_native_vector_failure_when_main_wraps_then_display_and_original_source_are_preserved() {
        use bkmr::domain::repositories::vector_repository::VectorRepository;
        use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;
        use bkmr::application::error::ApplicationError;
        use bkmr::domain::error::DomainError;
        use bkmr::cli::error::CliError;

        // SQLite itself executes this real missing-table query. This material
        // is in-memory; no filesystem source or persistent authority is claimed.
        let repository = SqliteVectorRepository::new(":memory:").unwrap();
        let failure = repository.has_embeddings().unwrap_err();
        let native = std::error::Error::source(&failure).unwrap().downcast_ref::<rusqlite::Error>().unwrap();
        let pointer = native as *const rusqlite::Error;
        let cli = CliError::from(failure).context("actual native main context");
        let expected = format!("Command execution failed: {cli}");
        let wrapped = command_execution_error(cli);
        assert_eq!(wrapped.to_string(), expected);
        let cli = wrapped.source().unwrap().downcast_ref::<CliError>().unwrap();
        let application = std::error::Error::source(cli).unwrap().downcast_ref::<ApplicationError>().unwrap();
        let domain = std::error::Error::source(application).unwrap().downcast_ref::<DomainError>().unwrap();
        let native = std::error::Error::source(domain).unwrap().downcast_ref::<rusqlite::Error>().unwrap();
        assert_eq!(native as *const rusqlite::Error, pointer);
        assert_eq!(native.sqlite_error_code(), Some(rusqlite::ErrorCode::Unknown));
    }

}
