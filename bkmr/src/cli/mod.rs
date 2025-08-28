// bkmr/src/cli/mod.rs
use crate::cli::args::{Cli, Commands};
use crate::cli::error::CliResult;
use crate::infrastructure::di::ServiceContainer;
use crate::config::Settings;
use termcolor::StandardStream;

pub mod args;
pub mod bookmark_commands;
pub mod command_handler;
pub mod completion;
pub mod display;
pub mod error;
pub mod fzf;
pub mod process;
pub mod tag_commands;

// Old execute_command removed - use execute_command_with_services with dependency injection

pub fn execute_command_with_services(
    stderr: StandardStream, 
    cli: Cli, 
    services: ServiceContainer,
    settings: &Settings,
) -> CliResult<()> {
    if cli.generate_config {
        println!("{}", crate::config::generate_default_config());
        return Ok(());
    }
    match cli.command {
        Some(Commands::Search { .. }) => {
            let handler = command_handler::SearchCommandHandler::with_services(services, settings.clone());
            handler.execute(cli)
        }
        Some(Commands::SemSearch { .. }) => bookmark_commands::semantic_search(stderr, cli, &services),
        Some(Commands::Open { .. }) => bookmark_commands::open(cli, &services),
        Some(Commands::Add { .. }) => bookmark_commands::add(cli, &services),
        Some(Commands::Delete { .. }) => bookmark_commands::delete(cli, &services),
        Some(Commands::Update { .. }) => bookmark_commands::update(cli, &services),
        Some(Commands::Edit { .. }) => bookmark_commands::edit(cli, &services, settings),
        Some(Commands::Show { .. }) => bookmark_commands::show(cli, &services),
        Some(Commands::Tags { .. }) => tag_commands::show_tags(cli, &services),
        Some(Commands::Surprise { .. }) => bookmark_commands::surprise(cli, &services),
        Some(Commands::CreateDb { .. }) => bookmark_commands::create_db(cli, &services, settings),
        Some(Commands::SetEmbeddable { .. }) => bookmark_commands::set_embeddable(cli, &services),
        Some(Commands::Backfill { .. }) => bookmark_commands::backfill(cli, &services),
        Some(Commands::LoadTexts { .. }) => bookmark_commands::load_texts(cli, &services),
        Some(Commands::LoadJson { .. }) => bookmark_commands::load_json(cli, &services),
        Some(Commands::ImportFiles { .. }) => bookmark_commands::import_files(cli, &services),
        Some(Commands::Info { .. }) => bookmark_commands::info(cli, &services, settings),
        Some(Commands::Completion { shell }) => handle_completion(shell),
        Some(Commands::Lsp { no_interpolation }) => handle_lsp(settings, no_interpolation),
        Some(Commands::Xxx { ids, tags }) => {
            eprintln!("ids: {:?}, tags: {:?}", ids, tags);
            Ok(())
        }
        None => Ok(()),
    }
}

fn handle_completion(shell: String) -> CliResult<()> {
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
    match completion::generate_completion(&shell) {
        Ok(_) => Ok(()),
        Err(e) => Err(error::CliError::CommandFailed(format!(
            "Failed to generate completion script: {}",
            e
        ))),
    }
}

fn handle_lsp(settings: &Settings, no_interpolation: bool) -> CliResult<()> {
    use tokio::runtime::Runtime;

    // Create a tokio runtime for the LSP server
    let rt = Runtime::new().map_err(|e| {
        error::CliError::CommandFailed(format!("Failed to create async runtime: {}", e))
    })?;

    // Run the LSP server
    rt.block_on(async {
        crate::lsp::run_lsp_server(settings, no_interpolation).await;
    });

    Ok(())
}
