//! LSP snippet service adapter
//!
//! Provides async wrapper around bkmr bookmark service for snippet operations

use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::TemplateService;
use crate::domain::repositories::query::BookmarkQuery;
use crate::lsp::domain::{Snippet, SnippetFilter};
use crate::util::interpolation::InterpolationHelper;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Result type for snippet operations
pub type SnippetResult<T> = Result<T, SnippetError>;

/// Errors that can occur during snippet operations
#[derive(Debug, thiserror::Error)]
pub enum SnippetError {
    #[error("Application error: {0}")]
    Application(#[from] crate::application::error::ApplicationError),
    #[error("Service error: {0}")]
    Service(String),
}

/// LSP snippet service that adapts bkmr bookmark service for async LSP usage
#[derive(Debug)]
pub struct LspSnippetService {
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
}

impl LspSnippetService {
    // Remove factory-based constructor - use dependency injection only

    /// Create with specific services (for testing)
    pub fn with_services(
        bookmark_service: Arc<dyn BookmarkService>,
        template_service: Arc<dyn TemplateService>,
    ) -> Self {
        Self {
            bookmark_service,
            template_service,
        }
    }

    /// Create with a specific bookmark service (for testing) - backward compatibility
    pub fn with_service(bookmark_service: Arc<dyn BookmarkService>) -> Self {
        use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
        use crate::application::services::TemplateServiceImpl;
        use std::sync::Arc;
        
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(template_engine));
        
        Self {
            bookmark_service,
            template_service,
        }
    }
}

#[async_trait]
pub trait AsyncSnippetService: Send + Sync {
    /// Fetch snippets based on the provided filter
    async fn fetch_snippets(&self, filter: &SnippetFilter) -> SnippetResult<Vec<Snippet>>;

    /// Health check to verify the service is working
    async fn health_check(&self) -> SnippetResult<()>;
}

#[async_trait]
impl AsyncSnippetService for LspSnippetService {
    #[instrument(skip(self))]
    async fn fetch_snippets(&self, filter: &SnippetFilter) -> SnippetResult<Vec<Snippet>> {
        debug!("Fetching snippets with filter: {:?}", filter);

        // Use tokio::task::spawn_blocking to run sync code in async context
        let bookmark_service = Arc::clone(&self.bookmark_service);
        let filter_clone = filter.clone();

        let bookmarks = tokio::task::spawn_blocking(move || {
            // Build the bookmark query based on the snippet filter
            let mut query = BookmarkQuery::new();

            // Build the text query combining FTS and prefix search
            let mut text_parts = Vec::new();

            // Add FTS query if we have one
            if let Some(fts_query) = filter_clone.build_fts_query() {
                text_parts.push(fts_query);
            }

            // Add prefix search if specified
            if let Some(ref prefix) = filter_clone.query_prefix {
                if !prefix.trim().is_empty() {
                    // Use title prefix search for better snippet matching
                    text_parts.push(format!("metadata:{}*", prefix));
                }
            }

            // Combine all text parts with AND logic
            if !text_parts.is_empty() {
                let combined_query = if text_parts.len() == 1 {
                    text_parts.into_iter().next().unwrap()
                } else {
                    text_parts.join(" AND ")
                };
                query = query.with_text_query(Some(&combined_query));
            }

            // Set limit
            if filter_clone.max_results > 0 {
                query = query.with_limit(Some(filter_clone.max_results));
            }

            debug!("Executing bookmark search with query: {:?}", query);
            bookmark_service.search_bookmarks(&query)
        })
        .await
        .map_err(|e| SnippetError::Service(format!("Task join error: {}", e)))?
        .map_err(SnippetError::Application)?;

        debug!("Found {} bookmarks", bookmarks.len());

        // Apply interpolation and convert bookmarks to snippets
        let template_service = Arc::clone(&self.template_service);
        let enable_interpolation = filter.enable_interpolation;
        
        let snippets: Vec<Snippet> = bookmarks
            .into_iter()
            .map(|bookmark| {
                // Apply interpolation if enabled
                let content = if enable_interpolation {
                    // Apply interpolation using InterpolationHelper (same pattern as actions)
                    match InterpolationHelper::render_if_needed(
                        &bookmark.url, // URL field contains snippet content
                        &bookmark,
                        &template_service,
                        "lsp snippet",
                    ) {
                        Ok(interpolated) => {
                            debug!(
                                "Template interpolation successful for bookmark: {}",
                                bookmark.title
                            );
                            interpolated
                        }
                        Err(e) => {
                            debug!(
                                "Template interpolation failed for bookmark {}: {}, using raw content",
                                bookmark.title, e
                            );
                            // Fallback to raw content on interpolation error
                            bookmark.url.clone()
                        }
                    }
                } else {
                    debug!(
                        "Template interpolation disabled for bookmark: {}",
                        bookmark.title
                    );
                    bookmark.url.clone()
                };

                Snippet {
                    id: bookmark.id.unwrap_or(0), // Handle Option<i32>
                    title: bookmark.title,        // Use title field, not metadata
                    content,                      // Now contains processed content
                    description: bookmark.description,
                    tags: bookmark
                        .tags
                        .into_iter()
                        .map(|tag| tag.value().to_string())
                        .collect(), // Use value() method
                    access_count: bookmark.access_count,
                }
            })
            .collect();

        debug!("Converted to {} snippets", snippets.len());
        Ok(snippets)
    }

    #[instrument(skip(self))]
    async fn health_check(&self) -> SnippetResult<()> {
        debug!("Performing health check");

        let bookmark_service = Arc::clone(&self.bookmark_service);

        // Try to perform a simple query to verify the service is working
        tokio::task::spawn_blocking(move || {
            let query = BookmarkQuery::new().with_limit(Some(1));
            bookmark_service.search_bookmarks(&query)
        })
        .await
        .map_err(|e| SnippetError::Service(format!("Health check task failed: {}", e)))?
        .map_err(SnippetError::Application)?;

        debug!("Health check passed");
        Ok(())
    }
}

// Remove Default implementation - require explicit dependency injection
