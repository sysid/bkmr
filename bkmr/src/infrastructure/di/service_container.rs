use crate::application::actions::{
    DefaultAction, EnvAction, MarkdownAction, ShellAction, SnippetAction, TextAction, UriAction,
};
use crate::application::services::action_service::{ActionService, ActionServiceImpl};
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::tag_service::TagService;
use crate::application::services::interpolation_service::InterpolationService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, InterpolationServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::config::Settings;
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::{ActionResolver, SystemTagActionResolver};
use crate::domain::embedding::Embedder;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::embeddings::{DummyEmbedding, OpenAiEmbedding};
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::file_import_repository::FileImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crate::application::error::ApplicationResult;
use std::path::Path;
use std::sync::Arc;
use crossterm::style::Stylize;

/// Production service container - single source of truth for service creation
pub struct ServiceContainer {
    // Core services
    pub bookmark_repository: Arc<SqliteBookmarkRepository>,
    pub embedder: Arc<dyn Embedder>,
    pub bookmark_service: Arc<dyn BookmarkService>,
    pub tag_service: Arc<dyn TagService>,
    pub action_service: Arc<dyn ActionService>,
    
    // Utility services
    pub clipboard_service: Arc<dyn ClipboardService>,
    pub interpolation_service: Arc<dyn InterpolationService>,
    pub template_service: Arc<dyn TemplateService>,
}

impl ServiceContainer {
    /// Create all services with explicit dependency injection
    pub fn new(config: &Settings, openai: bool) -> ApplicationResult<Self> {
        // Base infrastructure
        let bookmark_repository = Self::create_repository(&config.db_url)?;
        let embedder = Self::create_embedder(openai)?;
        let clipboard_service = Arc::new(ClipboardServiceImpl::new());
        let interpolation_service = Self::create_interpolation_service();
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
        
        let action_service = Self::create_action_service(
            &bookmark_repository,
            &interpolation_service,
            &(clipboard_service.clone() as Arc<dyn ClipboardService>),
            &embedder,
            config
        )?;
        
        Ok(Self {
            bookmark_repository,
            embedder,
            bookmark_service,
            tag_service,
            action_service,
            clipboard_service,
            interpolation_service,
            template_service,
        })
    }
    
    fn create_repository(db_url: &str) -> ApplicationResult<Arc<SqliteBookmarkRepository>> {
        // Check if the database file exists before trying to create the repository
        if !Path::new(db_url).exists() {
            eprintln!("{}", format!("Error: Database not found at '{}'", db_url).red());
            eprintln!("The configured database does not exist.");
            eprintln!("");
            eprintln!("You can either:");
            eprintln!("  1. Create the database using 'bkmr create-db {}'", db_url);
            eprintln!("  2. Set BKMR_DB_URL environment variable to point to an existing database");
            eprintln!("  3. Update the db_url in your config file (~/.config/bkmr/config.toml)");
            std::process::exit(1);
        }

        // Create the repository, runs all migrations
        let repository = SqliteBookmarkRepository::from_url(db_url)
            .map_err(|e| crate::application::error::ApplicationError::Other(
                format!("Failed to create SQLite bookmark repository: {}", e)
            ))?;
        
        Ok(Arc::new(repository))
    }
    
    fn create_embedder(openai: bool) -> ApplicationResult<Arc<dyn Embedder>> {
        if openai {
            Ok(Arc::new(OpenAiEmbedding::default()))
        } else {
            Ok(Arc::new(DummyEmbedding))
        }
    }
    
    fn create_interpolation_service() -> Arc<dyn InterpolationService> {
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        Arc::new(InterpolationServiceImpl::new(interpolation_engine))
    }
    
    fn create_template_service() -> Arc<dyn TemplateService> {
        Arc::new(TemplateServiceImpl::new())
    }
    
    fn create_action_service(
        repository: &Arc<SqliteBookmarkRepository>,
        interpolation_service: &Arc<dyn InterpolationService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
        config: &Settings,
    ) -> ApplicationResult<Arc<dyn ActionService>> {
        let resolver = Self::create_action_resolver(
            repository, interpolation_service, clipboard_service, embedder, config
        )?;
        Ok(Arc::new(ActionServiceImpl::new(resolver, repository.clone())))
    }
    
    fn create_action_resolver(
        repository: &Arc<SqliteBookmarkRepository>,
        interpolation_service: &Arc<dyn InterpolationService>,
        clipboard_service: &Arc<dyn ClipboardService>,
        embedder: &Arc<dyn Embedder>,
        config: &Settings,
    ) -> ApplicationResult<Arc<dyn ActionResolver>> {
        // Create all actions with explicit dependencies
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
            config.shell_opts.interactive,
        ));
        
        let markdown_action: Box<dyn BookmarkAction> = 
            Box::new(MarkdownAction::new_with_repository(repository.clone(), embedder.clone()));
            
        let env_action: Box<dyn BookmarkAction> = 
            Box::new(EnvAction::new(interpolation_service.clone()));
            
        let default_action: Box<dyn BookmarkAction> = 
            Box::new(DefaultAction::new(interpolation_service.clone()));
        
        Ok(Arc::new(SystemTagActionResolver::new(
            uri_action,
            snippet_action, 
            text_action,
            shell_action,
            markdown_action,
            env_action,
            default_action,
        )))
    }
}

impl std::fmt::Debug for ServiceContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceContainer")
            .field("bookmark_repository", &"Arc<SqliteBookmarkRepository>")
            .field("embedder", &"Arc<dyn Embedder>")
            .field("bookmark_service", &"Arc<dyn BookmarkService>")
            .field("tag_service", &"Arc<dyn TagService>")
            .field("action_service", &"Arc<dyn ActionService>")
            .field("clipboard_service", &"Arc<dyn ClipboardService>")
            .field("interpolation_service", &"Arc<dyn InterpolationService>")
            .field("template_service", &"Arc<dyn TemplateService>")
            .finish()
    }
}