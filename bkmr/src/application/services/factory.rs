// src/application/services/factory.rs
use crate::app_state::AppState;
use crate::application::actions::{
    DefaultAction, EnvAction, MarkdownAction, ShellAction, SnippetAction, TextAction, UriAction,
};
use crate::application::services::action_service::{ActionService, ActionServiceImpl};
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::tag_service::TagService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::{ActionResolver, SystemTagActionResolver};
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::file_import_repository::FileImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use crossterm::style::Stylize;
use std::path::Path;
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
        Arc::new(FileImportRepository::new()),
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
    let shell_executor = Arc::new(SafeShellExecutor::new());
    let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
    Arc::new(TemplateServiceImpl::new(template_engine))
}

// Interpolation functionality is now part of TemplateService
// This function is kept for backward compatibility during transition
pub fn create_interpolation_service() -> Arc<dyn TemplateService> {
    create_template_service()
}

// Create an action resolver with default implementations for each system tag
pub fn create_action_resolver() -> Arc<dyn ActionResolver> {
    // Create the template service (which includes interpolation) and clipboard service
    let template_service = create_template_service();
    let clipboard_service = create_clipboard_service();
    let repository = create_bookmark_repository();

    // Create actions for each system tag
    let uri_action: Box<dyn BookmarkAction> =
        Box::new(UriAction::new(Arc::clone(&template_service)));

    let snippet_action: Box<dyn BookmarkAction> = Box::new(SnippetAction::new(
        Arc::clone(&clipboard_service),
        Arc::clone(&template_service),
    ));

    let text_action: Box<dyn BookmarkAction> = Box::new(TextAction::new(
        Arc::clone(&clipboard_service),
        Arc::clone(&template_service),
    ));

    let app_state = AppState::read_global();
    let shell_action: Box<dyn BookmarkAction> = Box::new(ShellAction::new(
        Arc::clone(&template_service),
        app_state.settings.shell_opts.interactive,
    ));

    // Always create MarkdownAction with repository
    // The action itself will determine whether to update embeddings based on:
    // 1. OpenAI embeddings being available
    // 2. The bookmark having embeddable=true
    let markdown_action: Box<dyn BookmarkAction> = Box::new(MarkdownAction::new_with_repository(
        repository.clone(),
    ));

    let env_action: Box<dyn BookmarkAction> =
        Box::new(EnvAction::new(Arc::clone(&template_service)));

    let default_action: Box<dyn BookmarkAction> =
        Box::new(DefaultAction::new(Arc::clone(&template_service)));

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
