// src/application/actions/default_action.rs
use crate::application::services::interpolation::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct DefaultAction {
    interpolation_service: Arc<dyn InterpolationService>,
}

impl DefaultAction {
    pub fn new(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
        }
    }
}

impl BookmarkAction for DefaultAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Default action falls back to treating the bookmark as a URI

        // Render the URL with interpolation if needed
        let rendered_url = self
            .interpolation_service
            .render_bookmark_url(bookmark)
            .map_err(|e| DomainError::Other(format!("Failed to render URL: {}", e)))?;

        // Open the URL in default browser/application
        debug!("Opening with default application: {}", rendered_url);
        open::that(&rendered_url)
            .map_err(|e| DomainError::Other(format!("Failed to open: {}", e)))?;

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Open with default application"
    }
}
