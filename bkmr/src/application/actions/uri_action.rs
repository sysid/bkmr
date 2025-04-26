// src/application/actions/uri_action.rs
use crate::application::services::interpolation::InterpolationService;
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

        // Open the URL
        self.open_url(&rendered_url)
    }

    fn description(&self) -> &'static str {
        "Open in browser or application"
    }
}
