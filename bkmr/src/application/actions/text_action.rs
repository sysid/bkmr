// src/application/actions/text_action.rs
use crate::application::services::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::domain::services::clipboard::ClipboardService;
use crate::util::interpolation::InterpolationHelper;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug)]
pub struct TextAction {
    clipboard_service: Arc<dyn ClipboardService>,
    interpolation_service: Arc<dyn InterpolationService>,
}

impl TextAction {
    pub fn new(
        clipboard_service: Arc<dyn ClipboardService>,
        interpolation_service: Arc<dyn InterpolationService>,
    ) -> Self {
        Self {
            clipboard_service,
            interpolation_service,
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
        let rendered_content = InterpolationHelper::render_if_needed(
            content,
            bookmark,
            &self.interpolation_service,
            "text",
        )?;

        // Copy to clipboard
        self.clipboard_service
            .copy_to_clipboard(&rendered_content)?;

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Copy text to clipboard"
    }
}
