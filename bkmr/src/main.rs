// bkmr/src/main.rs

use clap::Parser;
use crossterm::style::Stylize;
use termcolor::{ColorChoice, StandardStream};
use tracing::{debug, info};
use tracing_subscriber::{
    filter::{filter_fn, LevelFilter},
    fmt::{self, format::FmtSpan},
    prelude::*,
};

use bkmr::adapter::embeddings::{Context, DummyAi, OpenAi};
use bkmr::cli::args::{Cli, Commands};
use bkmr::cli::commands;
use bkmr::environment::CONFIG;
use bkmr::helper::is_env_var_set;
use bkmr::CTX;

fn main() {
    // let stdout = StandardStream::stdout(ColorChoice::Always);
    // use stderr as human output in order to make stdout output passable to downstream processes
    let stderr = StandardStream::stderr(ColorChoice::Always);

    let cli = Cli::parse();

    setup_logging(cli.debug);

    if let Some(Commands::CreateDb { .. }) = &cli.command {
        // Skip the path.exists check and create database with correct schema
    } else {
        let path = std::path::Path::new(&CONFIG.db_url);
        if !path.exists() {
            eprintln!("Error: db_url path does not exist: {:?}", CONFIG.db_url);
            std::process::exit(1);
        }
        let _ = commands::enable_embeddings_if_required(); // migrate db
    }

    if cli.openai {
        if !is_env_var_set("OPENAI_API_KEY") {
            println!("Environment variable OPENAI_API_KEY is not set.");
            std::process::exit(1);
        }

        info!("Using OpenAI API");
        CTX.set(Context::new(Box::<OpenAi>::default())).unwrap();
    } else {
        info!("Using DummyAI");
        CTX.set(Context::new(Box::new(DummyAi))).unwrap();
    }

    if let Err(e) = commands::execute_command(stderr, cli) {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    }
}

fn setup_logging(verbosity: u8) {
    let filter = match verbosity {
        0 => LevelFilter::WARN,
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        _ => {
            eprintln!("Don't be crazy, max is -d -d");
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
        .with_writer(std::io::stderr)  // Set writer first
        .with_target(true)
        .with_thread_names(false)
        .with_span_events(FmtSpan::CLOSE);

    // Apply filters to the layer
    let filtered_layer = fmt_layer
        .with_filter(filter)
        .with_filter(module_filter);

    tracing_subscriber::registry()
        .with(filtered_layer)
        .init();

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
    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::fixture;

    #[ctor::ctor]
    fn init() {
        setup_logging(2); // Set maximum debug level for tests
    }

    #[fixture]
    fn temp_dir() -> Utf8PathBuf {
        let tempdir = tempdir().unwrap();
        let options = dir::CopyOptions::new().overwrite(true);
        copy_items(
            &[
                "tests/resources/bkmr.v1.db",
                "tests/resources/bkmr.v2.db",
                "tests/resources/bkmr.v2.noembed.db",
            ],
            "../db",
            &options,
        )
        .expect("Failed to copy test project directory");

        tempdir.into_path()
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}