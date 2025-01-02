// bkmr/src/main.rs
use anyhow::Result;
use std::process;

use clap::Parser;
use crossterm::style::Stylize;
use log::{debug, info};
use stdext::function_name;
use termcolor::{ColorChoice, StandardStream};

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

    set_logger(&cli);

    if let Some(Commands::CreateDb { .. }) = &cli.command {
        // Skip the path.exists check and create database with correct schema
    } else {
        let path = std::path::Path::new(&CONFIG.db_url);
        if !path.exists() {
            eprintln!("Error: db_url path does not exist: {:?}", CONFIG.db_url);
            process::exit(1);
        }
        commands::enable_embeddings_if_required(); // migrate db
    }

    if cli.openai {
        if !is_env_var_set("OPENAI_API_KEY") {
            println!("Environment variable OPENAI_API_KEY is not set.");
            process::exit(1);
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

fn set_logger(cli: &Cli) {
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::Warn)
                .try_init();
        }
        1 => {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::Info)
                .filter_module("skim", log::LevelFilter::Info)
                .filter_module("tuikit", log::LevelFilter::Info)
                .filter_module("html5ever", log::LevelFilter::Info)
                .filter_module("reqwest", log::LevelFilter::Info)
                .filter_module("mio", log::LevelFilter::Info)
                .filter_module("want", log::LevelFilter::Info)
                .try_init();
            info!("Debug mode: info");
        }
        2 => {
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::max())
                .filter_module("skim", log::LevelFilter::Info)
                .filter_module("tuikit", log::LevelFilter::Info)
                .filter_module("html5ever", log::LevelFilter::Info)
                .filter_module("reqwest", log::LevelFilter::Info)
                .filter_module("mio", log::LevelFilter::Info)
                .filter_module("want", log::LevelFilter::Info)
                .try_init();
            debug!("Debug mode: debug");
        }
        _ => {
            eprintln!("Don't be crazy, max is -d -d");
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::max())
                .filter_module("skim", log::LevelFilter::Info)
                .filter_module("tuikit", log::LevelFilter::Info)
                .filter_module("html5ever", log::LevelFilter::Info)
                .filter_module("reqwest", log::LevelFilter::Info)
                .filter_module("mio", log::LevelFilter::Info)
                .filter_module("want", log::LevelFilter::Info)
                .try_init();
            debug!("Debug mode: debug");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::{fixture, rstest};

    use super::*;
    use bkmr::cli::args::Cli;
    use bkmr::cli::commands::{find_similar, randomized, sem_search};
    use bkmr::model::bms::Bookmarks;

    #[ctor::ctor]
    fn init() {
        let _ = env_logger::builder()
            // Include all events in tests
            .filter_level(log::LevelFilter::max())
            .filter_module("skim", log::LevelFilter::Info)
            .filter_module("tuikit", log::LevelFilter::Info)
            .filter_module("reqwest", log::LevelFilter::Info)
            // Ensure events are captured by `cargo test`
            .is_test(true)
            // Ignore errors initializing the logger if tests race to configure it
            .try_init();
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
