// bkmr/src/main.rs
use anyhow::Result;
use std::fs::create_dir_all;
use std::io::Write;
use std::path::PathBuf;
use std::process;

use anyhow::anyhow;
use camino::Utf8Path;
use clap::{Parser, Subcommand};
use crossterm::style::Stylize;
use diesel::connection::SimpleConnection;
use diesel::result::DatabaseErrorKind;
use diesel::result::Error::DatabaseError;
use diesel_migrations::MigrationHarness;
use inquire::Confirm;
use itertools::Itertools;
use log::{debug, error, info};
use stdext::function_name;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use bkmr::adapter::dal::Dal;
use bkmr::adapter::embeddings::{
    cosine_similarity, deserialize_embedding, Context, DummyAi, OpenAi,
};
use bkmr::adapter::json::{bms_to_json, read_ndjson_file_and_create_bookmarks};
use bkmr::cli::args::{Cli, Commands};
use bkmr::cli::commands;
use bkmr::environment::CONFIG;
use bkmr::helper::{confirm, ensure_int_vector, init_db, is_env_var_set, MIGRATIONS};
use bkmr::model::bms::Bookmarks;
use bkmr::model::bookmark::BookmarkUpdater;
use bkmr::model::bookmark::{Bookmark, BookmarkBuilder};
use bkmr::model::tag::Tags;
use bkmr::service::embeddings::create_embeddings_for_non_bookmarks;
use bkmr::service::fzf::fzf_process;
use bkmr::service::process::{
    delete_bms, edit_bms, open_bm, process, show_bms, DisplayBookmark, DisplayField, ALL_FIELDS,
    DEFAULT_FIELDS,
};
use bkmr::CTX;
use bkmr::{dlog2, load_url_details};

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

    if let Err(e) = execute_command(stderr, cli) {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    }
}

fn execute_command(stderr: StandardStream, cli: Cli) -> Result<()> {
    match cli.command {
        Some(Commands::Search {
            fts_query,
            tags_exact,
            tags_all,
            tags_all_not,
            tags_any,
            tags_any_not,
            tags_prefix,
            order_desc,
            order_asc,
            non_interactive,
            is_fuzzy,
            is_json,
            limit,
        }) => {
            commands::search_bookmarks(
                tags_prefix,
                tags_all,
                fts_query,
                tags_any,
                tags_all_not,
                tags_any_not,
                tags_exact,
                order_desc,
                order_asc,
                is_fuzzy,
                is_json,
                limit,
                non_interactive,
                stderr,
            )
        }
        Some(Commands::SemSearch {
            query,
            limit,
            non_interactive,
        }) => commands::sem_search(query, limit, non_interactive, stderr),
        Some(Commands::Open { ids }) => commands::open_bookmarks(ids),
        Some(Commands::Add {
            url,
            tags,
            title,
            desc,
            no_web,
            edit,
        }) => commands::add_bookmark(url, tags, title, desc, no_web, edit),
        Some(Commands::Delete { ids }) => commands::delete_bookmarks(ids),
        Some(Commands::Update {
            ids,
            tags,
            tags_not,
            force,
        }) => commands::update_bookmarks(force, tags, tags_not, ids),
        Some(Commands::Edit { ids }) => commands::edit_bookmarks(ids),
        Some(Commands::Show { ids }) => commands::show_bookmarks(ids),
        Some(Commands::Tags { tag }) => commands::show_tags(tag),
        Some(Commands::CreateDb { path }) => commands::create_db(path),
        Some(Commands::Surprise { n }) => commands::randomized(n),
        Some(Commands::Backfill { dry_run }) => commands::backfill_embeddings(dry_run),
        Some(Commands::LoadTexts { dry_run, path }) => commands::load_texts(dry_run, path),
        Some(Commands::Xxx { ids, tags }) => {
            eprintln!(
                "({}:{}) ids: {:?}, tags: {:?}",
                function_name!(),
                line!(),
                ids,
                tags
            );
            Ok(())
        }
        None => Ok(()),
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

    #[allow(unused_variables)]
    #[ignore = "currently only works in isolation"]
    #[rstest]
    fn test_find_similar_when_embed_null(temp_dir: Utf8PathBuf) {
        // Given: v2 database with embeddings and OpenAI context
        fs::rename("../db/bkmr.v2.noembed.db", "../db/bkmr.db").expect("Failed to rename database");
        let bms = Bookmarks::new("".to_string());
        CTX.set(Context::new(Box::<OpenAi>::default())).unwrap();

        // When: find similar for "blub"
        let results = find_similar(&"blub".to_string(), &bms).unwrap();

        // Then: Expect no findings
        assert_eq!(results.len(), 0);
    }

    #[allow(unused_variables)]
    #[rstest]
    fn test_find_similar(temp_dir: Utf8PathBuf) {
        // Given: v2 database with embeddings and OpenAi context
        fs::rename("../db/bkmr.v2.db", "../db/bkmr.db").expect("Failed to rename database");
        let bms = Bookmarks::new("".to_string());
        CTX.set(Context::new(Box::<OpenAi>::default())).unwrap();

        // When: find similar for "blub"
        let results = find_similar(&"blub".to_string(), &bms).unwrap();

        // Then: Expect the first three entries to be: blub, blub3, blub2
        assert_eq!(results.len(), 11);
        // Extract the first element (id) of the first three tuples
        let first_three_ids: Vec<_> = results.into_iter().take(3).map(|(id, _)| id).collect();
        assert_eq!(first_three_ids, vec![4, 6, 5]);
    }

    #[allow(unused_variables)]
    #[ignore = "interactive: visual check required"]
    #[rstest]
    fn test_sem_search_via_visual_check(temp_dir: Utf8PathBuf) {
        let stderr = StandardStream::stderr(ColorChoice::Always);
        fs::rename("../db/bkmr.v2.db", "../db/bkmr.db").expect("Failed to rename database");
        // this is only visible test
        CTX.set(Context::new(Box::<OpenAi>::default())).unwrap();
        // Given: v2 database with embeddings
        // When:
        sem_search("blub".to_string(), None, false, stderr);
        // Then: Expect the first three entries to be: blub, blub3, blub2
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[ignore = "interactive: opens browser link"]
    #[test]
    fn test_randomized() {
        randomized(1);
    }
}
