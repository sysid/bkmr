// src/application/services/interpolation_service.rs
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::interface::InterpolationEngine;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::instrument;

/// Service interface for interpolation-related operations
pub trait InterpolationService: Send + Sync + Debug {
    /// Render an interpolated URL within the context of a bookmark
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String>;
}

/// Implementation of InterpolationService using a template engine
#[derive(Debug)]
pub struct InterpolationServiceImpl {
    pub interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl InterpolationServiceImpl {
    pub fn new(template_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            interpolation_engine: template_engine,
        }
    }
}

impl InterpolationService for InterpolationServiceImpl {
    #[instrument(level = "debug", skip(self, bookmark))]
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String> {
        self.interpolation_engine
            .render_bookmark_url(bookmark)
            .map_err(|e| ApplicationError::Other(format!("Template rendering error: {}", e)))
    }
}
