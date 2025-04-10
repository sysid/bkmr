// src/application/services/template_service.rs
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::interface::InterpolationEngine;
use std::sync::Arc;
use tracing::instrument;

pub struct InterpolationService {
    pub(crate) interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl InterpolationService {
    pub fn new(template_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            interpolation_engine: template_engine,
        }
    }

    #[instrument(level = "debug", skip(self, bookmark))]
    pub fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String> {
        self.interpolation_engine
            .render_bookmark_url(bookmark)
            .map_err(|e| ApplicationError::Other(format!("Template rendering error: {}", e)))
    }

    #[instrument(level = "debug", skip(self))]
    pub fn render_url(&self, url: &str) -> ApplicationResult<String> {
        self.interpolation_engine
            .render_url(url)
            .map_err(|e| ApplicationError::Other(format!("Template rendering error: {}", e)))
    }
}
