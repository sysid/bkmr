//! Test context abstraction for consistent test setup and isolation
//!
//! Provides a unified way to create isolated test environments with proper
//! database setup, service configuration, and resource management.

use std::collections::HashSet;
use std::sync::Arc;

use crate::application::services::{BookmarkService, TagService};
use crate::domain::bookmark::Bookmark;
use crate::domain::embedding::Embedder;
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crate::lsp::services::command_service::CommandService;
use crate::lsp::services::completion_service::CompletionService;
use crate::lsp::services::document_service::DocumentService;
use crate::lsp::services::snippet_service::LspSnippetService;
use crate::util::test_service_container::TestServiceContainer;

/// Bundle of LSP services pre-configured for testing
#[derive(Debug)]
pub struct LspServiceBundle {
    pub snippet_service: Arc<LspSnippetService>,
    pub completion_service: CompletionService,
    pub command_service: CommandService,
    pub document_service: DocumentService,
}

/// Comprehensive test context providing isolated database and pre-configured services
///
/// This abstraction ensures consistent test setup across the entire codebase,
/// eliminates boilerplate, and provides proper isolation for database-accessing tests.
/// 
/// Now delegates to TestServiceContainer for centralized service management.
#[derive(Debug)]
pub struct TestContext {
    container: TestServiceContainer,
}

impl TestContext {
    /// Create a new isolated test context
    ///
    /// This sets up:
    /// - Test environment with proper logging and configuration
    /// - Isolated test database with fresh schema
    /// - Dummy embedding provider for consistent behavior
    /// - Pre-configured bookmark service
    /// - Template service for interpolation
    pub fn new() -> Self {
        let container = TestServiceContainer::new();
        Self { container }
    }

    /// Get the bookmark service
    pub fn bookmark_service(&self) -> Arc<dyn BookmarkService> {
        self.container.bookmark_service.clone()
    }

    /// Get the repository
    pub fn repository(&self) -> Arc<SqliteBookmarkRepository> {
        self.container.bookmark_repository.clone()
    }

    /// Get the embedder
    pub fn embedder(&self) -> Arc<dyn Embedder> {
        self.container.embedder.clone()
    }

    /// Create tag service with test configuration  
    pub fn create_tag_service(&self) -> Arc<dyn TagService> {
        self.container.tag_service.clone()
    }

    /// Create template service with test configuration
    pub fn create_template_service(
        &self,
    ) -> Arc<dyn crate::application::services::TemplateService> {
        self.container.template_service.clone()
    }

    /// Create LSP command service with test configuration
    pub fn create_command_service(&self) -> CommandService {
        CommandService::with_service(self.container.bookmark_service.clone())
    }

    /// Create complete LSP service bundle for integration testing
    pub fn create_lsp_services(&self) -> LspServiceBundle {
        let bundle = self.container.create_lsp_services();
        
        // Convert from TestServiceContainer's LspServiceBundle to TestContext's LspServiceBundle
        LspServiceBundle {
            snippet_service: bundle.snippet_service,
            completion_service: bundle.completion_service,
            command_service: bundle.command_service,
            document_service: bundle.document_service,
        }
    }

    /// Create a bookmark with the test embedder
    ///
    /// This avoids global state dependencies in domain object creation
    pub fn create_bookmark(
        &self,
        url: &str,
        title: &str,
        description: &str,
        tags: HashSet<Tag>,
    ) -> Result<Bookmark, crate::domain::error::DomainError> {
        Bookmark::new(url, title, description, tags, self.container.embedder.as_ref())
    }

    /// Create a simple bookmark with default values for testing
    pub fn create_test_bookmark(&self, title: &str) -> Bookmark {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        self.create_bookmark(
            &format!(
                "https://example.com/{}",
                title.replace(' ', "-").to_lowercase()
            ),
            title,
            &format!("Test bookmark: {}", title),
            tags,
        )
        .unwrap()
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_new_test_context_when_created_then_bookmark_service_accessible() {
        let ctx = TestContext::new();
        assert!(ctx.bookmark_service().get_all_bookmarks(None, None).is_ok());
    }

    #[test]
    fn given_test_context_when_create_test_bookmark_then_returns_configured_bookmark() {
        let ctx = TestContext::new();
        let bookmark = ctx.create_test_bookmark("Test Bookmark");
        assert_eq!(bookmark.title, "Test Bookmark");
        assert!(bookmark.url.contains("test-bookmark"));
    }

    #[tokio::test]
    async fn given_test_context_when_create_lsp_services_then_all_services_healthy() {
        use crate::lsp::services::snippet_service::AsyncSnippetService;

        let ctx = TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();

        // Verify all services are properly configured
        assert!(lsp_bundle.snippet_service.health_check().await.is_ok());
        assert!(lsp_bundle.completion_service.health_check().await.is_ok());
    }

    // Example of non-database test that can run in parallel
    #[test]
    fn given_tag_value_when_create_tag_then_returns_tag_with_value() {
        let tag = Tag::new("test").unwrap();
        assert_eq!(tag.value(), "test");
    }
}
