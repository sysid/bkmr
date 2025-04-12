use std::path::Path;
// src/application/services/factory.rs
use crate::app_state::AppState;
use crate::application::actions::{DefaultAction, EnvAction, MarkdownAction, ShellAction, SnippetAction, TextAction, UriAction};
use crate::application::services::action_service::{ActionService, ActionServiceImpl};
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::interpolation::InterpolationService;
use crate::application::services::tag_service::TagService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::{ActionResolver, SystemTagActionResolver};
use crate::domain::search::SemanticSearch;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::json_import_repository::JsonImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crossterm::style::Stylize;
use std::sync::{Arc, OnceLock};

// cache repository to avoid multiple migrations
static REPOSITORY_INSTANCE: OnceLock<Arc<SqliteBookmarkRepository>> = OnceLock::new();

// Modified to use the cached repository
pub fn create_bookmark_repository() -> Arc<SqliteBookmarkRepository> {
    REPOSITORY_INSTANCE
        .get_or_init(|| {
            let app_state = AppState::read_global();
            let db_url = &app_state.settings.db_url;

            // Check if the database file exists before trying to create the repository
            if !Path::new(db_url).exists() {
                eprintln!("{}", "Error: Database not found.".red());
                eprintln!("No database configured or the configured database does not exist.");
                eprintln!("Either:");
                eprintln!(
                    "  1. Set BKMR_DB_URL environment variable to point to an existing database"
                );
                eprintln!("  2. Create a database using 'bkmr create-db <path>'");
                eprintln!("  3. Ensure the default database at '~/.config/bkmr/bkmr.db' exists");
                std::process::exit(1);
            }

            // Create the repository only once, runs all migrations
            Arc::new(
                SqliteBookmarkRepository::from_url(db_url)
                    .expect("Failed to create SQLite bookmark repository"),
            )
        })
        .clone()
}

/// Creates a bookmark service with the default repository and embedder
pub fn create_bookmark_service() -> Arc<dyn BookmarkService> {
    let app_state = AppState::read_global();
    let embedder = Arc::clone(&app_state.context.embedder); // Now already Arc<dyn Embedder>
    let repository = create_bookmark_repository();

    // Create and return the service
    Arc::new(BookmarkServiceImpl::new(
        repository.clone(),
        embedder,
        Arc::new(JsonImportRepository::new()),
    ))
}

/// Creates a tag service with the default repository
pub fn create_tag_service() -> Arc<dyn TagService> {
    let repository = create_bookmark_repository();

    // Create and return the service
    Arc::new(TagServiceImpl::new(repository.clone()))
}

pub fn create_clipboard_service() -> Arc<dyn ClipboardService> {
    Arc::new(ClipboardServiceImpl::new())
}

pub fn create_template_service() -> Arc<dyn TemplateService> {
    Arc::new(TemplateServiceImpl::new())
}

pub fn create_interpolation_service() -> InterpolationService {
    let shell_executor = Arc::new(SafeShellExecutor::new());
    let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
    InterpolationService::new(template_engine)
}

pub fn create_semantic_search(query: &str, limit: Option<usize>) -> SemanticSearch {
    SemanticSearch::new(query, limit)
}

// Create an action resolver with default implementations for each system tag
pub fn create_action_resolver() -> Arc<dyn ActionResolver> {
    // Create the interpolation engine and clipboard service
    let interpolation_service = create_interpolation_service();
    let clipboard_service = create_clipboard_service();
    let repository = create_bookmark_repository();

    // Create actions for each system tag
    let uri_action: Box<dyn BookmarkAction> = Box::new(UriAction::new(Arc::clone(
        &interpolation_service.interpolation_engine,
    )));

    let snippet_action: Box<dyn BookmarkAction> = Box::new(SnippetAction::new(
        Arc::clone(&clipboard_service),
        Arc::clone(&interpolation_service.interpolation_engine),
    ));

    let text_action: Box<dyn BookmarkAction> = Box::new(TextAction::new(
        Arc::clone(&clipboard_service),
        Arc::clone(&interpolation_service.interpolation_engine),
    ));

    let shell_action: Box<dyn BookmarkAction> = Box::new(ShellAction::new(Arc::clone(
        &interpolation_service.interpolation_engine,
    )));

    // Always create MarkdownAction with repository
    // The action itself will determine whether to update embeddings based on:
    // 1. OpenAI embeddings being available
    // 2. The bookmark having embeddable=true
    let markdown_action: Box<dyn BookmarkAction> = Box::new(MarkdownAction::new_with_repository(
        Arc::clone(&interpolation_service.interpolation_engine),
        repository.clone(),
    ));

    let env_action: Box<dyn BookmarkAction> = Box::new(EnvAction::new(Arc::clone(
        &interpolation_service.interpolation_engine,
    )));

    let default_action: Box<dyn BookmarkAction> = Box::new(DefaultAction::new(Arc::clone(
        &interpolation_service.interpolation_engine,
    )));

    // Create and return the resolver
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

// Create the action service
pub fn create_action_service() -> Arc<dyn ActionService> {
    let repository = create_bookmark_repository();
    let resolver = create_action_resolver();

    Arc::new(ActionServiceImpl::new(resolver, repository))
}
