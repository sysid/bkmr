// src/application/actions/snippet_action.rs
use crate::application::services::template_service::TemplateService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::domain::services::clipboard::ClipboardService;
use crate::util::interpolation::InterpolationHelper;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct SnippetAction {
    clipboard_service: Arc<dyn ClipboardService>,
    template_service: Arc<dyn TemplateService>,
}

impl SnippetAction {
    pub fn new(
        clipboard_service: Arc<dyn ClipboardService>,
        template_service: Arc<dyn TemplateService>,
    ) -> Self {
        debug!("Creating new SnippetAction");
        Self {
            clipboard_service,
            template_service,
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
        let rendered_content = InterpolationHelper::render_if_needed(
            content,
            bookmark,
            &self.template_service,
            "snippet",
        )?;

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
