use crate::application::actions::{
    DefaultAction, EnvAction, MarkdownAction, ShellAction, SnippetAction, TextAction, UriAction,
};
use crate::application::services::action_service::{ActionService, ActionServiceImpl};
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::tag_service::TagService;
use crate::application::services::interpolation_service::InterpolationService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, InterpolationServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::{ActionResolver, SystemTagActionResolver};
use crate::domain::embedding::Embedder;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::file_import_repository::FileImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crate::util::testing::{init_test_env, setup_test_db};
use crate::lsp::services::{CommandService, CompletionService, DocumentService, LspSnippetService};
use std::sync::Arc;

/// Test service container with isolated dependencies
/// IMPORTANT: Tests still run single-threaded due to shared SQLite database
pub struct TestServiceContainer {
    pub bookmark_service: Arc<dyn BookmarkService>,
    pub tag_service: Arc<dyn TagService>, 
    pub action_service: Arc<dyn ActionService>,
    pub interpolation_service: Arc<dyn InterpolationService>,
    pub template_service: Arc<dyn TemplateService>,
    pub clipboard_service: Arc<dyn ClipboardService>,
}

impl TestServiceContainer {
    /// Create test services with isolated database
    /// NOTE: Database is still shared across tests - single-threaded execution required
    pub fn new() -> Self {
        let _env = init_test_env();
        
        // Create test repository (shared SQLite instance)
        let repository = Arc::new(setup_test_db());
        let embedder: Arc<dyn Embedder> = Arc::new(DummyEmbedding);
        let clipboard_service = Arc::new(ClipboardServiceImpl::new());
        let interpolation_service = Self::create_test_interpolation_service();
        let template_service = Self::create_test_template_service();
        
        // Application services with test dependencies
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository.clone(),
            embedder.clone(),
            Arc::new(FileImportRepository::new()),
        ));
        
        let tag_service = Arc::new(TagServiceImpl::new(repository.clone()));
        
        let action_service = Self::create_test_action_service(
            &repository, 
            &interpolation_service, 
            &(clipboard_service.clone() as Arc<dyn ClipboardService>),
            &embedder
        );
        
        Self {
            bookmark_service,
            tag_service,
            action_service,
            interpolation_service,
            template_service,
            clipboard_service,
        }
    }
    
    fn create_test_interpolation_service() -> Arc<dyn InterpolationService> {
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        Arc::new(InterpolationServiceImpl::new(interpolation_engine))
    }
    
    fn create_test_template_service() -> Arc<dyn TemplateService> {
        Arc::new(TemplateServiceImpl::new())
    }
    
    fn create_test_action_service(
        repository: &Arc<SqliteBookmarkRepository>,
        interpolation_service: &Arc<dyn InterpolationService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
    ) -> Arc<dyn ActionService> {
        // Create test action resolver with mock dependencies
        let resolver = Self::create_test_action_resolver(
            repository, interpolation_service, clipboard_service, embedder
        );
        Arc::new(ActionServiceImpl::new(resolver, repository.clone()))
    }
    
    fn create_test_action_resolver(
        repository: &Arc<SqliteBookmarkRepository>,
        interpolation_service: &Arc<dyn InterpolationService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
    ) -> Arc<dyn ActionResolver> {
        // Create all actions with explicit dependencies - test configuration
        let uri_action: Box<dyn BookmarkAction> = 
            Box::new(UriAction::new(interpolation_service.clone()));
            
        let snippet_action: Box<dyn BookmarkAction> = Box::new(SnippetAction::new(
            clipboard_service.clone(),
            interpolation_service.clone(),
        ));
        
        let text_action: Box<dyn BookmarkAction> = Box::new(TextAction::new(
            clipboard_service.clone(),
            interpolation_service.clone(),
        ));
        
        let shell_action: Box<dyn BookmarkAction> = Box::new(ShellAction::new(
            interpolation_service.clone(),
            true, // Test with interactive mode enabled
        ));
        
        let markdown_action: Box<dyn BookmarkAction> = 
            Box::new(MarkdownAction::new_with_repository(repository.clone(), embedder.clone()));
            
        let env_action: Box<dyn BookmarkAction> = 
            Box::new(EnvAction::new(interpolation_service.clone()));
            
        let default_action: Box<dyn BookmarkAction> = 
            Box::new(DefaultAction::new(interpolation_service.clone()));
        
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
    
    /// Create LSP services for integration testing
    pub fn create_lsp_services(&self) -> LspTestBundle {
        let snippet_service = Arc::new(LspSnippetService::with_services(
            self.bookmark_service.clone(),
            self.interpolation_service.clone(),
        ));
        
        LspTestBundle {
            completion_service: CompletionService::new(snippet_service.clone()),
            command_service: CommandService::with_service(self.bookmark_service.clone()),
            document_service: DocumentService::new(),
        }
    }
}

pub struct LspTestBundle {
    pub completion_service: CompletionService,
    pub command_service: CommandService,
    pub document_service: DocumentService,
}