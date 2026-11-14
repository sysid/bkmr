// bkmr/src/cli/mod.rs
use crate::cli::args::{Cli, Commands};
use crate::cli::error::CliResult;
use crate::config::Settings;
use crate::infrastructure::di::ServiceContainer;

pub mod args;
pub mod backup_restore;
pub mod bookmark_commands;
pub mod command_handler;
pub mod completion;
pub mod display;
pub mod error;
pub mod fzf;
pub mod history_diff;
pub mod hsearch_handler;
pub mod process;
pub mod skill;
pub mod tag_commands;

// Old execute_command removed - use execute_command_with_services with dependency injection

pub fn execute_command_with_services(
    cli: Cli,
    services: ServiceContainer,
    settings: &Settings,
) -> CliResult<()> {
    match cli.command {
        Some(Commands::Search { .. }) => {
            let handler =
                command_handler::SearchCommandHandler::with_services(services, settings.clone());
            handler.execute(cli)
        }
        Some(Commands::HSearch { .. }) => hsearch_handler::hybrid_search(cli, &services, settings),
        Some(Commands::SemSearch { .. }) => bookmark_commands::semantic_search(cli, &services),
        Some(Commands::Open { .. }) => bookmark_commands::open(
            cli,
            services.bookmark_service.clone(),
            services.action_service.clone(),
            services.interpolation_service.clone(),
        ),
        Some(Commands::Add { .. }) => bookmark_commands::add(
            cli,
            services.bookmark_service.clone(),
            services.template_service.clone(),
        ),
        Some(Commands::Delete { .. }) => {
            bookmark_commands::delete(cli, services.bookmark_service.clone())
        }
        Some(Commands::History { .. }) => {
            bookmark_commands::history(cli, services.bookmark_service.clone(), settings)
        }
        Some(Commands::Restore { .. }) => {
            bookmark_commands::restore(cli, services.bookmark_service.clone(), settings)
        }
        Some(Commands::Verify { .. }) => {
            bookmark_commands::verify(cli, services.bookmark_service.clone())
        }
        Some(Commands::Update { .. }) => bookmark_commands::update(
            cli,
            services.bookmark_service.clone(),
            services.tag_service.clone(),
        ),
        Some(Commands::Edit { .. }) => bookmark_commands::edit(
            cli,
            services.bookmark_service.clone(),
            services.template_service.clone(),
            settings,
        ),
        Some(Commands::Show { .. }) => bookmark_commands::show(cli, &services, settings),
        Some(Commands::Tags { .. }) => tag_commands::show_tags(cli, &services),
        Some(Commands::Surprise { .. }) => bookmark_commands::surprise(cli, &services),
        Some(Commands::Backfill { .. }) => bookmark_commands::backfill(cli, &services),
        Some(Commands::ClearEmbeddings { .. }) => {
            bookmark_commands::clear_embeddings(cli, &services)
        }
        Some(Commands::LoadJson { .. }) => bookmark_commands::load_json(cli, &services),
        Some(Commands::ImportFiles { .. }) => {
            bookmark_commands::import_files(cli, &services, settings)
        }
        Some(Commands::Info { .. }) => bookmark_commands::info(cli, &services, settings),
        Some(Commands::Lsp { no_interpolation }) => handle_lsp(&services, no_interpolation),
        // main.rs handles these before any database is opened; reaching here is a dispatch bug.
        Some(Commands::CreateDb { .. } | Commands::Completion { .. } | Commands::Skill { .. }) => {
            Err(error::CliError::CommandFailed(
                "database-independent command should be handled in main.rs".to_string(),
            ))
        }
        None => Ok(()),
    }
}

/// Runs the LSP server on this command's services, so the database is opened and migrated once
fn handle_lsp(services: &ServiceContainer, no_interpolation: bool) -> CliResult<()> {
    use tokio::runtime::Runtime;

    // Create a tokio runtime for the LSP server
    let rt = Runtime::new().map_err(|e| {
        error::CliError::CommandFailed(format!("Failed to create async runtime: {}", e))
    })?;

    rt.block_on(crate::lsp::run_lsp_server(
        services.bookmark_service.clone(),
        services.interpolation_service.clone(),
        no_interpolation,
    ))
    .map_err(error::CliError::CommandFailed)
}
