// src/application/services/action_service.rs
use crate::domain::action::BookmarkAction;
use crate::domain::action_resolver::ActionResolver;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::repository::BookmarkRepository;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Service for executing actions on bookmarks
pub trait ActionService {
    /// Executes the default action for a bookmark
    fn execute_default_action(&self, bookmark: &Bookmark) -> DomainResult<()>;

    /// Executes the default action for a bookmark by ID
    fn execute_default_action_by_id(&self, id: i32) -> DomainResult<()>;

    /// Gets a description of the default action for a bookmark
    fn get_default_action_description(&self, bookmark: &Bookmark) -> &'static str;
}

/// Implementation of ActionService that uses an ActionResolver
pub struct ActionServiceImpl<R: BookmarkRepository> {
    resolver: Arc<dyn ActionResolver>,
    repository: Arc<R>,
}

impl<R: BookmarkRepository> ActionServiceImpl<R> {
    pub fn new(resolver: Arc<dyn ActionResolver>, repository: Arc<R>) -> Self {
        Self { resolver, repository }
    }
}

impl<R: BookmarkRepository> ActionService for ActionServiceImpl<R> {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute_default_action(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // First, record the access (increase access count)
        if let Some(id) = bookmark.id {
            debug!("Recording access for bookmark {}", id);
            self.record_bookmark_access(id)?;
        }

        // Resolve and execute the appropriate action
        let action = self.resolver.resolve_action(bookmark);
        debug!("Executing action: {}", action.description());
        action.execute(bookmark)
    }

    #[instrument(skip(self), level = "debug")]
    fn execute_default_action_by_id(&self, id: i32) -> DomainResult<()> {
        // Get the bookmark
        let bookmark = self.repository.get_by_id(id)?
            .ok_or_else(|| DomainError::BookmarkNotFound(id.to_string()))?;

        // Execute the default action
        self.execute_default_action(&bookmark)
    }

    fn get_default_action_description(&self, bookmark: &Bookmark) -> &'static str {
        let action = self.resolver.resolve_action(bookmark);
        action.description()
    }
}

// Helper methods
impl<R: BookmarkRepository> ActionServiceImpl<R> {
    // Record that a bookmark was accessed
    #[instrument(skip(self), level = "trace")]
    fn record_bookmark_access(&self, id: i32) -> DomainResult<()> {
        // Get the bookmark
        let mut bookmark = self.repository.get_by_id(id)?
            .ok_or_else(|| DomainError::BookmarkNotFound(id.to_string()))?;

        // Record access
        bookmark.record_access();

        // Update in repository
        self.repository.update(&bookmark)?;

        Ok(())
    }
}
