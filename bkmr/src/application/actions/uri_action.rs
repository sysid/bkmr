// src/application/actions/uri_action.rs
use crate::application::services::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
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

        // The old way to store a command. Refused rather than handed to the OS opener, which
        // would fail with an error that says nothing about the cause.
        if url.starts_with("shell::") {
            return Err(DomainError::Other(
                "'shell::' bookmarks are no longer run: store the command without 'shell::' and \
                 tag the bookmark _shell_"
                    .to_string(),
            ));
        }

        // Handle regular URLs or file paths
        if let Some(path) = crate::util::path::abspath(url) {
            debug!("Resolved path: {}", path);

            // Check if it's a markdown file
            if path.ends_with(".md") {
                debug!("Opening markdown file with editor: {}", path);
                let editor = crate::util::editor::configured_editor();
                crate::util::editor::edit_file(&editor, std::path::Path::new(&path))
                    .map_err(DomainError::Other)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::InterpolationServiceImpl;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SystemShellExecutor,
    };
    use std::collections::HashSet;

    #[test]
    fn given_legacy_shell_url_when_execute_then_refuses_with_migration_hint_and_runs_nothing() {
        // Arrange: `shell::` was the old way to store a command, replaced by the _shell_ type
        let engine = Arc::new(MiniJinjaEngine::new(Arc::new(SystemShellExecutor::new())));
        let action = UriAction::new(Arc::new(InterpolationServiceImpl::new(engine)));
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let url = format!("shell::touch '{}'", marker.display());
        let bookmark = Bookmark::new(url.as_str(), "legacy", "", HashSet::new()).unwrap();

        // Act
        let result = action.execute(&bookmark);

        // Assert
        let err = result.expect_err("shell:: is no longer run").to_string();
        assert!(err.contains("_shell_"), "got: {err}");
        assert!(!marker.exists(), "the command must not run");
    }
}
