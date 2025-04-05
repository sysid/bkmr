// bkmr/src/cli/mod.rs
use crate::cli::args::{Cli, Commands};
use crate::cli::error::CliResult;
use termcolor::StandardStream;

pub mod args;
pub mod bookmark_commands;
pub mod completion;
pub mod display;
pub mod error;
pub mod fzf;
pub mod process;
pub mod tag_commands;

pub fn execute_command(stderr: StandardStream, cli: Cli) -> CliResult<()> {
    if cli.generate_config {
        println!("{}", crate::config::generate_default_config());
        return Ok(());
    }
    match cli.command {
        Some(Commands::Search { .. }) => bookmark_commands::search(stderr, cli),
        Some(Commands::SemSearch { .. }) => bookmark_commands::semantic_search(stderr, cli),
        Some(Commands::Open { .. }) => bookmark_commands::open(cli),
        Some(Commands::Add { .. }) => bookmark_commands::add(cli),
        Some(Commands::Delete { .. }) => bookmark_commands::delete(cli),
        Some(Commands::Update { .. }) => bookmark_commands::update(cli),
        Some(Commands::Edit { .. }) => bookmark_commands::edit(cli),
        Some(Commands::Show { .. }) => bookmark_commands::show(cli),
        Some(Commands::Tags { .. }) => tag_commands::show_tags(cli),
        Some(Commands::Surprise { .. }) => bookmark_commands::surprise(cli),
        Some(Commands::CreateDb { .. }) => bookmark_commands::create_db(cli),
        Some(Commands::SetEmbeddable { .. }) => bookmark_commands::set_embeddable(cli),
        Some(Commands::Backfill { .. }) => bookmark_commands::backfill(cli),
        Some(Commands::LoadTexts { .. }) => bookmark_commands::load_texts(cli),
        Some(Commands::LoadJson { .. }) => bookmark_commands::load_json(cli),
        Some(Commands::Info { .. }) => bookmark_commands::info(cli),
        Some(Commands::Completion { shell }) => handle_completion(shell),
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
