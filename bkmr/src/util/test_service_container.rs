// src/util/test_service_container.rs
use crate::application::services::action_service::{ActionService, ActionServiceImpl};
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::tag_service::TagService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::{ActionResolver, SystemTagActionResolver};
use crate::domain::embedding::Embedder;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::file_import_repository::FileImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crate::lsp::backend::BkmrConfig;
use crate::lsp::services::snippet_service::LspSnippetService;
use crate::util::testing::{EnvGuard};
use std::sync::Arc;

/// Test service container - single source of truth for test service creation
/// Provides the same services as production ServiceContainer but with test-specific configuration
pub struct TestServiceContainer {
    // Environment isolation
    pub _env_guard: EnvGuard,
    
    // Core services
    pub bookmark_repository: Arc<SqliteBookmarkRepository>,
    pub embedder: Arc<dyn Embedder>,
    pub bookmark_service: Arc<dyn BookmarkService>,
    pub tag_service: Arc<dyn TagService>,
    pub action_service: Arc<dyn ActionService>,
    
    // Utility services
    pub clipboard_service: Arc<dyn ClipboardService>,
    pub template_service: Arc<dyn TemplateService>,
}

impl TestServiceContainer {
    /// Create all test services with explicit dependency injection
    /// Equivalent to ServiceContainer::new but with test-specific setup
    pub fn new() -> Self {
        // Initialize test environment (includes logging setup)
        let env_guard = EnvGuard::new();
        
        // Create test database with proper migrations (shared test database)
        let bookmark_repository = Self::create_shared_test_db();
        let embedder = Self::create_test_embedder();
        let clipboard_service = Arc::new(ClipboardServiceImpl::new());
        let template_service = Self::create_template_service();
        
        // Application services with explicit DI
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            bookmark_repository.clone(),
            embedder.clone(),
            Arc::new(FileImportRepository::new()),
        ));
        
        let tag_service = Arc::new(TagServiceImpl::new(
            bookmark_repository.clone()
        ));
        
        let action_service = Self::create_test_action_service(
            &bookmark_repository,
            &template_service,
            &(clipboard_service.clone() as Arc<dyn ClipboardService>),
            &embedder,
        );
        
        Self {
            _env_guard: env_guard,
            bookmark_repository,
            embedder,
            bookmark_service,
            tag_service,
            action_service,
            clipboard_service,
            template_service,
        }
    }
    
    /// Create shared test database (same as original setup_test_db approach)
    fn create_shared_test_db() -> Arc<SqliteBookmarkRepository> {
        // Use the same approach as the original setup_test_db function
        Arc::new(crate::util::testing::setup_test_db())
    }
    
    /// Create test embedder (always dummy for consistent test behavior)
    fn create_test_embedder() -> Arc<dyn Embedder> {
        Arc::new(DummyEmbedding)
    }
    
    /// Create template service (same logic as production ServiceContainer)
    fn create_template_service() -> Arc<dyn TemplateService> {
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        Arc::new(TemplateServiceImpl::new(template_engine))
    }
    
    /// Create action service for tests with test configuration
    fn create_test_action_service(
        repository: &Arc<SqliteBookmarkRepository>,
        template_service: &Arc<dyn TemplateService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
    ) -> Arc<dyn ActionService> {
        let resolver = Self::create_test_action_resolver(
            repository, template_service, clipboard_service, embedder
        );
        Arc::new(ActionServiceImpl::new(resolver, repository.clone()))
    }
    
    /// Create action resolver for tests with test-friendly settings
    fn create_test_action_resolver(
        repository: &Arc<SqliteBookmarkRepository>,
        template_service: &Arc<dyn TemplateService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
    ) -> Arc<dyn ActionResolver> {
        // Create all actions with explicit dependencies - using test-friendly settings
        let uri_action: Box<dyn BookmarkAction> = 
            Box::new(crate::application::actions::UriAction::new(template_service.clone()));
            
        let snippet_action: Box<dyn BookmarkAction> = Box::new(crate::application::actions::SnippetAction::new(
            clipboard_service.clone(),
            template_service.clone(),
        ));
        
        let text_action: Box<dyn BookmarkAction> = Box::new(crate::application::actions::TextAction::new(
            clipboard_service.clone(),
            template_service.clone(),
        ));
        
        // Non-interactive shell action for tests
        let shell_action: Box<dyn BookmarkAction> = Box::new(crate::application::actions::ShellAction::new(
            template_service.clone(),
            false, // non-interactive for tests
        ));
        
        let markdown_action: Box<dyn BookmarkAction> = 
            Box::new(crate::application::actions::MarkdownAction::new_with_repository(repository.clone(), embedder.clone()));
            
        let env_action: Box<dyn BookmarkAction> = 
            Box::new(crate::application::actions::EnvAction::new(template_service.clone()));
            
        let default_action: Box<dyn BookmarkAction> = 
            Box::new(crate::application::actions::DefaultAction::new(template_service.clone()));
        
        Arc::new(SystemTagActionResolver::new(
            uri_action,
            snippet_action, 
            text_action,
            shell_action,
            markdown_action,
            env_action,
            default_action,
        ))
    }
    
    /// Create LSP services bundle for integration testing
    /// Returns pre-configured services ready for LSP backend testing
    pub fn create_lsp_services(&self) -> LspServiceBundle {
        let snippet_service = Arc::new(LspSnippetService::with_services(
            self.bookmark_service.clone(),
            self.template_service.clone(),
        ));
        
        let completion_service = crate::lsp::services::completion_service::CompletionService::with_config(
            snippet_service.clone(),
            BkmrConfig::default(),
        );
        
        let document_service = crate::lsp::services::document_service::DocumentService::new();
        
        let command_service = crate::lsp::services::command_service::CommandService::with_service(
            self.bookmark_service.clone()
        );
        
        LspServiceBundle {
            snippet_service,
            completion_service,
            document_service,
            command_service,
        }
    }
}

/// Bundle of LSP services for integration testing
/// Provides all LSP services pre-configured and ready to use
pub struct LspServiceBundle {
    pub snippet_service: Arc<LspSnippetService>,
    pub completion_service: crate::lsp::services::completion_service::CompletionService,
    pub document_service: crate::lsp::services::document_service::DocumentService,
    pub command_service: crate::lsp::services::command_service::CommandService,
}

impl Default for TestServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TestServiceContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestServiceContainer")
            .field("bookmark_repository", &"Arc<SqliteBookmarkRepository>")
            .field("embedder", &"Arc<dyn Embedder>")
            .field("bookmark_service", &"Arc<dyn BookmarkService>")
            .field("tag_service", &"Arc<dyn TagService>")
            .field("action_service", &"Arc<dyn ActionService>")
            .field("clipboard_service", &"Arc<dyn ClipboardService>")
            .field("template_service", &"Arc<dyn TemplateService>")
            .finish()
    }
}