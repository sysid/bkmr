//! LSP snippet service adapter
//!
//! Provides async wrapper around bkmr bookmark service for snippet operations

use async_trait::async_trait;
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::factory;
use crate::domain::repositories::query::BookmarkQuery;
use crate::lsp::domain::{Snippet, SnippetFilter};
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
}

impl LspSnippetService {
    /// Create a new LSP snippet service using the existing bookmark service
    pub fn new() -> Self {
        let bookmark_service = factory::create_bookmark_service();
        Self { bookmark_service }
    }

    /// Create with a specific bookmark service (for testing)
    pub fn with_service(bookmark_service: Arc<dyn BookmarkService>) -> Self {
        Self { bookmark_service }
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

        // Convert bookmarks to snippets
        let snippets: Vec<Snippet> = bookmarks
            .into_iter()
            .map(|bookmark| Snippet {
                id: bookmark.id.unwrap_or(0), // Handle Option<i32>
                title: bookmark.title, // Use title field, not metadata
                content: bookmark.url, // In bkmr, the URL field contains the actual content for snippets
                description: bookmark.description,
                tags: bookmark.tags.into_iter().map(|tag| tag.value().to_string()).collect(), // Use value() method
                access_count: bookmark.access_count,
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

impl Default for LspSnippetService {
    fn default() -> Self {
        Self::new()
    }
}