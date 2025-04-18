// src/cli/completion.rs
use crate::cli::args::Cli;
use clap::CommandFactory;
use clap_complete::{
    generate,
    shells::{Bash, Fish, Zsh},
};
use std::io::{self};
use tracing::{debug, instrument};

/// Generates shell completion scripts for the specified shell and prints to stdout.
///
/// # Arguments
///
/// * `shell` - The name of the shell to generate completions for: "bash", "zsh", or "fish"
///
/// # Returns
///
/// Returns Ok(()) on success, or an error if generation fails
///
/// # Examples
///
/// ```bash
/// # Generate Bash completions and save to a file
/// bkmr completion bash > ~/.bash_completion.d/bkmr
///
/// # Directly evaluate completions
/// eval "$(bkmr completion bash)"
/// ```
#[instrument(level = "debug")]
pub fn generate_completion(shell: &str) -> io::Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();

    match shell.to_lowercase().as_str() {
        "bash" => {
            debug!("Generating bash completion");
            generate(Bash, &mut cmd, bin_name, &mut io::stdout());
            Ok(())
        }
        "zsh" => {
            debug!("Generating zsh completion");
            generate(Zsh, &mut cmd, bin_name, &mut io::stdout());
            Ok(())
        }
        "fish" => {
            debug!("Generating fish completion");
            generate(Fish, &mut cmd, bin_name, &mut io::stdout());
            Ok(())
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Unsupported shell: {}. Supported shells: bash, zsh, fish",
                shell
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_generate_bash_completion() -> io::Result<()> {
        // Create a buffer to capture output
        let mut buffer = Vec::new();

        // Use a custom implementation of generate that writes to our buffer
        {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(Bash, &mut cmd, bin_name, &mut buffer);
        }

        // Verify the output has content
        assert!(!buffer.is_empty());

        // Convert to string and check content
        let content = String::from_utf8_lossy(&buffer);

        // Look for common elements in bash completion scripts
        assert!(
            content.contains("complete")
                || content.contains("compgen")
                || content.contains("COMPREPLY")
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_generate_zsh_completion() -> io::Result<()> {
        // Create a buffer to capture output
        let mut buffer = Vec::new();

        // Use a custom implementation of generate that writes to our buffer
        {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(Zsh, &mut cmd, bin_name, &mut buffer);
        }

        // Verify the output has content
        assert!(!buffer.is_empty());

        // Convert to string and check content
        let content = String::from_utf8_lossy(&buffer);

        // Look for common elements in zsh completion scripts
        assert!(
            content.contains("#compdef")
                || content.contains("_arguments")
                || content.contains("compdef")
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_generate_fish_completion() -> io::Result<()> {
        // Create a buffer to capture output
        let mut buffer = Vec::new();

        // Use a custom implementation of generate that writes to our buffer
        {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(Fish, &mut cmd, bin_name, &mut buffer);
        }

        // Verify the output has content
        assert!(!buffer.is_empty());

        // Convert to string and check content
        let content = String::from_utf8_lossy(&buffer);

        // Look for common elements in fish completion scripts
        assert!(content.contains("complete") && content.contains("-c bkmr"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_invalid_shell() {
        // Test with an invalid shell
        let result = generate_completion("invalid");

        // Verify the result is an error
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
            assert!(e.to_string().contains("Unsupported shell"));
        }
    }
}
