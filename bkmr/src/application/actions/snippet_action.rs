// src/application/actions/snippet_action.rs
use crate::application::services::interpolation::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::services::clipboard::ClipboardService;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct SnippetAction {
    clipboard_service: Arc<dyn ClipboardService>,
    interpolation_service: Arc<dyn InterpolationService>,
}

impl SnippetAction {
    pub fn new(
        clipboard_service: Arc<dyn ClipboardService>,
        interpolation_service: Arc<dyn InterpolationService>,
    ) -> Self {
        debug!("Creating new SnippetAction");
        Self {
            clipboard_service,
            interpolation_service,
        }
    }
}

impl BookmarkAction for SnippetAction {
    #[instrument(skip(self, bookmark), level = "debug", 
               fields(bookmark_id = ?bookmark.id, bookmark_title = %bookmark.title))]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the snippet content - this is stored in the URL field for snippet bookmarks
        let content = bookmark.snippet_content();

        // Apply any interpolation if the snippet contains template variables
        let rendered_content = if content.contains("{{") || content.contains("{%") {
            self.interpolation_service
                .render_bookmark_url(bookmark)
                .map_err(|e| DomainError::Other(format!("Failed to render snippet: {}", e)))?
        } else {
            content.to_string()
        };

        eprintln!("Copied to clipboard:\n{}", rendered_content);
        // Copy to clipboard
        self.clipboard_service
            .copy_to_clipboard(&rendered_content)?;

        // Optionally, we could print a confirmation message here, but that's UI logic
        // and should be handled at the CLI layer

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Copy to clipboard"
    }
}
