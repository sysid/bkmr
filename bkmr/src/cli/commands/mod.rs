// src/cli/commands/mod.rs
mod bookmark_commands;
mod tag_commands;

use crate::cli::args::{Cli, Commands};
use anyhow::Result;
use termcolor::StandardStream;
use crate::cli::error::CliResult;

pub fn execute_command(stderr: StandardStream, cli: Cli) -> CliResult<()> {
    match cli.command {
        // Bookmark commands
        Some(Commands::Search { .. }) => bookmark_commands::search(stderr, cli),
        Some(Commands::SemSearch { .. }) => bookmark_commands::semantic_search(stderr, cli),
        Some(Commands::Open { .. }) => bookmark_commands::open(cli),
        Some(Commands::Add { .. }) => bookmark_commands::add(cli),
        Some(Commands::Delete { .. }) => bookmark_commands::delete(cli),
        Some(Commands::Update { .. }) => bookmark_commands::update(cli),
        Some(Commands::Edit { .. }) => bookmark_commands::edit(cli),
        Some(Commands::Show { .. }) => bookmark_commands::show(cli),
        Some(Commands::Surprise { .. }) => bookmark_commands::surprise(cli),
        Some(Commands::Backfill { .. }) => bookmark_commands::backfill(cli),
        Some(Commands::LoadTexts { .. }) => bookmark_commands::load_texts(cli),

        // Tag commands
        Some(Commands::Tags { .. }) => tag_commands::show_tags(cli),

        // Database commands
        Some(Commands::CreateDb { .. }) => bookmark_commands::create_db(cli),

        // Debug/placeholder commands
        Some(Commands::Xxx { .. }) => {
            if let Commands::Xxx { ids, tags } = cli.command.unwrap() {
                eprintln!("ids: {:?}, tags: {:?}", ids, tags);
            }
            Ok(())
        },
        None => Ok(()),
    }
}