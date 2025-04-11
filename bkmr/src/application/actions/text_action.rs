// src/application/actions/text_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use crate::domain::services::clipboard::ClipboardService;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct TextAction {
    clipboard_service: Arc<dyn ClipboardService>,
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl TextAction {
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

impl BookmarkAction for TextAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // For text bookmarks, the behavior is similar to snippets
        // but the context and usage might be different

        // Get the content (stored in URL field for imported text)
        let content = &bookmark.url;

        // Apply any interpolation if the text contains template variables
        let rendered_content = if content.contains("{{") || content.contains("{%") {
            self.interpolation_engine.render_bookmark_url(bookmark)?
        } else {
            content.to_string()
        };

        // Copy to clipboard
        self.clipboard_service
            .copy_to_clipboard(&rendered_content)?;

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Copy text to clipboard"
    }
}
