// src/application/actions/uri_action.rs
use crate::application::services::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crossterm::style::Stylize;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct UriAction {
    interpolation_service: Arc<dyn InterpolationService>,
}

impl UriAction {
    pub fn new(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
        }
    }

    /// Execute a custom opener command with the URL as argument
    /// The opener command is expanded for ~ and environment variables,
    /// then executed via shell with the URL passed as $1
    #[instrument(skip(self), level = "debug")]
    fn execute_custom_opener(&self, opener: &str, url: &str) -> DomainResult<()> {
        // Expand ~ and environment variables in the opener path
        let expanded_opener = crate::util::path::expand_path(opener);
        debug!("Custom opener: {} -> {}", opener, expanded_opener);
        debug!("URL argument: {}", url);

        // Execute via shell: sh -c "$opener \"$1\"" -- <url>
        // This ensures proper quoting of the URL
        let mut child = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("{} \"$1\"", expanded_opener))
            .arg("--")
            .arg(url)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| {
                DomainError::Other(format!(
                    "Failed to execute custom opener '{}': {}",
                    expanded_opener, e
                ))
            })?;

        let status = child
            .wait()
            .map_err(|e| DomainError::Other(format!("Failed to wait on custom opener: {}", e)))?;

        if !status.success() {
            return Err(DomainError::Other(format!(
                "Custom opener '{}' exited with status: {}",
                expanded_opener,
                status.code().unwrap_or(-1)
            )));
        }

        debug!("Custom opener completed successfully");
        Ok(())
    }

    // Helper method to open a URL with proper rendering
    #[instrument(skip(self, url), level = "debug")]
    fn open_url(&self, url: &str) -> DomainResult<()> {
        debug!("Opening URL: {}", url);

        if url.starts_with("shell::") {
            // Extract the shell command
            let cmd = url.replace("shell::", "");
            eprintln!("Executing shell command: {}", cmd);
            eprintln!(
                "{}",
                "'shell::' is deprecated. Use SystemTag '_shell_' instead.".yellow()
            );

            // Create a child process with inherited stdio
            let mut child = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .map_err(|e| {
                    DomainError::Other(format!("Failed to execute shell command: {}", e))
                })?;

            // Wait for the process to complete
            let status = child
                .wait()
                .map_err(|e| DomainError::Other(format!("Failed to wait on command: {}", e)))?;

            debug!("Shell command exit status: {:?}", status);
            return Ok(());
        }

        // Handle regular URLs or file paths
        if let Some(path) = crate::util::path::abspath(url) {
            debug!("Resolved path: {}", path);

            // Check if it's a markdown file
            if path.ends_with(".md") {
                debug!("Opening markdown file with editor: {}", path);
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                debug!("Using editor: {}", editor);

                std::process::Command::new(editor)
                    .arg(&path)
                    .status()
                    .map_err(|e| {
                        DomainError::Other(format!("Failed to open with editor: {}", e))
                    })?;
            } else {
                debug!("Opening file with default OS application: {}", path);
                open::that(&path)
                    .map_err(|e| DomainError::Other(format!("Failed to open file: {}", e)))?;
            }
        } else {
            debug!("Opening URL with default OS command: {}", url);
            open::that(url)
                .map_err(|e| DomainError::Other(format!("Failed to open URL: {}", e)))?;
        }

        Ok(())
    }
}

impl BookmarkAction for UriAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // First record access
        // This should be done at the service level, not here
        // We'll add this to the action service

        // Render the URL with interpolation if needed
        let rendered_url = self
            .interpolation_service
            .render_bookmark_url(bookmark)
            .map_err(|e| DomainError::Other(format!("Failed to render URL: {}", e)))?;

        // Check for custom opener
        if let Some(opener) = &bookmark.opener {
            return self.execute_custom_opener(opener, &rendered_url);
        }

        // Open the URL with default behavior
        self.open_url(&rendered_url)
    }

    fn description(&self) -> &'static str {
        "Open in browser or application"
    }
}
