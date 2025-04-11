// src/application/actions/snippet_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use crate::domain::services::clipboard::ClipboardService;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct SnippetAction {
    clipboard_service: Arc<dyn ClipboardService>,
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl SnippetAction {
    pub fn new(
        clipboard_service: Arc<dyn ClipboardService>,
        interpolation_engine: Arc<dyn InterpolationEngine>,
    ) -> Self {
        Self {
            clipboard_service,
            interpolation_engine,
        }
    }
}

impl BookmarkAction for SnippetAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the snippet content - this is stored in the URL field for snippet bookmarks
        let content = bookmark.snippet_content();

        // Apply any interpolation if the snippet contains template variables
        let rendered_content = if content.contains("{{") || content.contains("{%") {
            self.interpolation_engine.render_bookmark_url(bookmark)?
        } else {
            content.to_string()
        };

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
