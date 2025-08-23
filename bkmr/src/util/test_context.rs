//! Test context abstraction for consistent test setup and isolation
//!
//! Provides a unified way to create isolated test environments with proper
//! database setup, service configuration, and resource management.

use std::sync::Arc;
use std::collections::HashSet;

use crate::application::services::{BookmarkService, BookmarkServiceImpl, TagService, TagServiceImpl};
use crate::domain::bookmark::Bookmark;
use crate::domain::embedding::Embedder;
use crate::domain::tag::Tag;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::infrastructure::repositories::file_import_repository::FileImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crate::lsp::services::command_service::CommandService;
use crate::lsp::services::completion_service::CompletionService;
use crate::lsp::services::document_service::DocumentService;
use crate::lsp::services::snippet_service::LspSnippetService;
use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};

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
#[derive(Debug)]
pub struct TestContext {
    _env_guard: EnvGuard,
    repository: Arc<SqliteBookmarkRepository>,
    embedder: Arc<dyn Embedder>,
    bookmark_service: Arc<dyn BookmarkService>,
}

impl TestContext {
    /// Create a new isolated test context
    /// 
    /// This sets up:
    /// - Test environment with proper logging and configuration
    /// - Isolated test database with fresh schema
    /// - Dummy embedding provider for consistent behavior
    /// - Pre-configured bookmark service
    pub fn new() -> Self {
        let _env = init_test_env();
        let _env_guard = EnvGuard::new();
        let repository = Arc::new(setup_test_db());
        let embedder = Arc::new(DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository.clone(),
            embedder.clone(),
            Arc::new(FileImportRepository::new()),
        ));

        Self {
            _env_guard,
            repository,
            embedder,
            bookmark_service,
        }
    }

    /// Get the bookmark service
    pub fn bookmark_service(&self) -> Arc<dyn BookmarkService> {
        self.bookmark_service.clone()
    }

    /// Get the repository
    pub fn repository(&self) -> Arc<SqliteBookmarkRepository> {
        self.repository.clone()
    }

    /// Get the embedder
    pub fn embedder(&self) -> Arc<dyn Embedder> {
        self.embedder.clone()
    }

    /// Create tag service with test configuration  
    pub fn create_tag_service(&self) -> Arc<dyn TagService> {
        Arc::new(TagServiceImpl::new(self.repository.clone()))
    }

    /// Create template service with test configuration
    pub fn create_template_service(&self) -> Arc<dyn crate::application::services::TemplateService> {
        use crate::application::services::factory;
        factory::create_template_service()
    }

    /// Create LSP command service with test configuration
    pub fn create_command_service(&self) -> CommandService {
        CommandService::with_service(self.bookmark_service.clone())
    }

    /// Create complete LSP service bundle for integration testing
    pub fn create_lsp_services(&self) -> LspServiceBundle {
        let snippet_service = Arc::new(LspSnippetService::with_service(self.bookmark_service.clone()));
        let completion_service = CompletionService::new(snippet_service.clone());
        let command_service = self.create_command_service();
        let document_service = DocumentService::new();

        LspServiceBundle {
            snippet_service,
            completion_service,
            command_service,
            document_service,
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
        Bookmark::new(url, title, description, tags, self.embedder.as_ref())
    }

    /// Create a simple bookmark with default values for testing
    pub fn create_test_bookmark(&self, title: &str) -> Bookmark {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());
        
        self.create_bookmark(
            &format!("https://example.com/{}", title.replace(' ', "-").to_lowercase()),
            title,
            &format!("Test bookmark: {}", title),
            tags,
        ).unwrap()
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
    fn test_context_creation() {
        let ctx = TestContext::new();
        assert!(ctx.bookmark_service().get_all_bookmarks(None, None).is_ok());
    }

    #[test]
    fn test_bookmark_creation_with_context() {
        let ctx = TestContext::new();
        let bookmark = ctx.create_test_bookmark("Test Bookmark");
        assert_eq!(bookmark.title, "Test Bookmark");
        assert!(bookmark.url.contains("test-bookmark"));
    }

    #[tokio::test]
    async fn test_lsp_services_bundle() {
        use crate::lsp::services::snippet_service::AsyncSnippetService;
        
        let ctx = TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        
        // Verify all services are properly configured
        assert!(lsp_bundle.snippet_service.health_check().await.is_ok());
        assert!(lsp_bundle.completion_service.health_check().await.is_ok());
    }

    // Example of non-database test that can run in parallel
    #[test]
    fn test_pure_domain_logic() {
        let tag = Tag::new("test").unwrap();
        assert_eq!(tag.value(), "test");
    }
}