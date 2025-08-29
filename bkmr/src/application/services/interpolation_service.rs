// src/application/services/interpolation_service.rs
use crate::application::error::ApplicationResult;
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::interface::InterpolationEngine;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::instrument;

pub trait InterpolationService: Send + Sync + Debug {
    /// Render an interpolated URL within the context of a bookmark
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String>;
}

#[derive(Debug)]
pub struct InterpolationServiceImpl {
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl Default for InterpolationServiceImpl {
    fn default() -> Self {
        // This is used only in tests, create a dummy engine
        use crate::infrastructure::interpolation::minijinja_engine::{
            MiniJinjaEngine, SafeShellExecutor,
        };
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        Self::new(engine)
    }
}

impl InterpolationServiceImpl {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            interpolation_engine,
        }
    }
}

impl InterpolationService for InterpolationServiceImpl {
    #[instrument(skip(self), level = "debug")]
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String> {
        self.interpolation_engine
            .render_bookmark_url(bookmark)
            .map_err(|e| crate::application::error::ApplicationError::Other(
                format!("Failed to render bookmark URL with interpolation: {}", e)
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bookmark::BookmarkBuilder;
    use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};

    fn create_test_service() -> InterpolationServiceImpl {
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        InterpolationServiceImpl::new(engine)
    }

    #[test]
    fn given_bookmark_when_render_url_then_returns_interpolated_url() {
        let service = create_test_service();
        let bookmark = BookmarkBuilder::default()
            .id(Some(1))
            .url("https://example.com/{{ env('HOME') }}".to_string())
            .title("Test".to_string())
            .description("Test".to_string())
            .tags(std::collections::HashSet::new())
            .access_count(0)
            .created_at(Some(chrono::Utc::now()))
            .updated_at(chrono::Utc::now())
            .embedding(None)
            .content_hash(None)
            .build().unwrap();

        let result = service.render_bookmark_url(&bookmark);

        assert!(result.is_ok());
    }

    #[test]
    fn given_bookmark_with_plain_url_when_render_then_returns_unchanged() {
        let service = create_test_service();
        let bookmark = BookmarkBuilder::default()
            .id(Some(2))
            .url("https://example.com/plain".to_string())
            .title("Test".to_string())
            .description("Test".to_string())
            .tags(std::collections::HashSet::new())
            .access_count(0)
            .created_at(Some(chrono::Utc::now()))
            .updated_at(chrono::Utc::now())
            .embedding(None)
            .content_hash(None)
            .build().unwrap();

        let result = service.render_bookmark_url(&bookmark);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://example.com/plain");
    }
}