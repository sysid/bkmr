// src/application/services/factory.rs
use std::sync::{Arc, OnceLock};

use crate::app_state::AppState;
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::interpolation::InterpolationService;
use crate::application::services::tag_service::TagService;
use crate::application::services::template_service::TemplateService;
use crate::application::{BookmarkServiceImpl, TagServiceImpl, TemplateServiceImpl};
use crate::domain::search::SemanticSearch;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::clipboard::ClipboardServiceImpl;
use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
use crate::infrastructure::repositories::json_import_repository::JsonImportRepository;
use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;

// cache repository to avoid multiple migrations
static REPOSITORY_INSTANCE: OnceLock<Arc<SqliteBookmarkRepository>> = OnceLock::new();

// Modified to use the cached repository
pub fn create_bookmark_repository() -> Arc<SqliteBookmarkRepository> {
    REPOSITORY_INSTANCE
        .get_or_init(|| {
            let app_state = AppState::read_global();
            let db_url = &app_state.settings.db_url;

            // Create the repository only once
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
