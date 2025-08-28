// src/application/services/action_service.rs
use crate::domain::action_resolver::ActionResolver;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::util::validation::ValidationHelper;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Service for executing actions on bookmarks
pub trait ActionService: Send + Sync {
    /// Executes the default action for a bookmark
    fn execute_default_action(&self, bookmark: &Bookmark) -> DomainResult<()>;

    /// Executes the default action for a bookmark by ID
    fn execute_default_action_by_id(&self, id: i32) -> DomainResult<()>;

    /// Executes the default action for a bookmark with override for interactive mode
    fn execute_default_action_with_options(
        &self,
        bookmark: &Bookmark,
        no_edit: bool,
        script_args: &[String],
    ) -> DomainResult<()>;

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
        Self {
            resolver,
            repository,
        }
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

    // todo: difference to default action execute
    #[instrument(skip(self), level = "debug")]
    fn execute_default_action_by_id(&self, id: i32) -> DomainResult<()> {
        // Get the bookmark
        let bookmark = ValidationHelper::validate_and_get_bookmark_domain(id, &*self.repository)?;

        // Execute the default action
        self.execute_default_action(&bookmark)
    }

    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute_default_action_with_options(
        &self,
        bookmark: &Bookmark,
        no_edit: bool,
        script_args: &[String],
    ) -> DomainResult<()> {
        use crate::application::actions::shell_action::ShellAction;
        use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
        use std::sync::Arc;
        use crate::domain::action::BookmarkAction;
        use crate::domain::system_tag::SystemTag;

        // First, record the access (increase access count)
        if let Some(id) = bookmark.id {
            debug!("Recording access for bookmark {}", id);
            self.record_bookmark_access(id)?;
        }

        // Check if this is a shell bookmark and no_edit is requested
        if no_edit && bookmark.tags.contains(&SystemTag::Shell.to_tag()?) {
            debug!("Executing shell action with no-edit mode");

            // Create a direct (non-interactive) shell action with script arguments
            let shell_executor = Arc::new(SafeShellExecutor::new());
            let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
            let interpolation_service = Arc::new(crate::application::TemplateServiceImpl::new(template_engine));
            let shell_action =
                ShellAction::new_direct_with_args(interpolation_service, script_args.to_vec());
            return shell_action.execute(bookmark);
        }

        // Otherwise, use the normal action resolver
        let action = self.resolver.resolve_action(bookmark);
        debug!("Executing action: {}", action.description());
        action.execute(bookmark) // todo: check interpolation
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
        let mut bookmark =
            ValidationHelper::validate_and_get_bookmark_domain(id, &*self.repository)?;

        bookmark.record_access();

        self.repository.update(&bookmark)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::action::BookmarkAction;
    use crate::domain::action_resolver::ActionResolver;
    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use crate::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
    use crate::util::testing::{init_test_env, EnvGuard};
    use std::collections::HashSet;
    use std::sync::Arc;

    // Mock action for testing
    #[derive(Debug)]
    struct MockAction {
        description: &'static str,
        executed: Arc<std::sync::Mutex<bool>>,
    }

    impl MockAction {
        fn new(description: &'static str) -> Self {
            Self {
                description,
                executed: Arc::new(std::sync::Mutex::new(false)),
            }
        }
    }

    impl BookmarkAction for MockAction {
        fn execute(&self, _bookmark: &Bookmark) -> DomainResult<()> {
            *self.executed.lock().unwrap() = true;
            Ok(())
        }

        fn description(&self) -> &'static str {
            self.description
        }
    }

    // Mock action resolver for testing
    #[derive(Debug)]
    struct MockActionResolver {
        action: Arc<MockAction>,
    }

    impl MockActionResolver {
        fn new(action: Arc<MockAction>) -> Self {
            Self { action }
        }
    }

    impl ActionResolver for MockActionResolver {
        fn resolve_action(&self, _bookmark: &Bookmark) -> Box<dyn BookmarkAction> {
            Box::new(MockAction::new(self.action.description))
        }
    }

    fn create_test_repository() -> Arc<SqliteBookmarkRepository> {
        // Use a unique in-memory database for ActionService tests to avoid interfering with other tests
        let db_url = ":memory:".to_string();
        let repository =
            SqliteBookmarkRepository::from_url(&db_url).expect("Could not create test repository");

        Arc::new(repository)
    }

    fn create_test_bookmark_with_shell_tag() -> Bookmark {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_shell_").unwrap());

        Bookmark {
            id: Some(1),
            url: "echo 'test script'".to_string(),
            title: "Test Shell Script".to_string(),
            description: "".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        }
    }

    fn create_test_bookmark_without_shell_tag() -> Bookmark {
        let tags = HashSet::new();

        Bookmark {
            id: Some(2),
            url: "https://example.com".to_string(),
            title: "Test URL".to_string(),
            description: "".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
        }
    }

    #[test]
    fn given_shell_bookmark_when_execute_default_action_with_no_edit_then_uses_direct_shell_action()
    {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();

        let repository = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let bookmark = create_test_bookmark_with_shell_tag();

        // Add bookmark to repository for access recording
        let mut bookmark_copy = bookmark.clone();
        repository.add(&mut bookmark_copy).unwrap();
        let stored_bookmark = repository.get_by_id(1).unwrap().unwrap();

        // Act
        let result = service.execute_default_action_with_options(&stored_bookmark, true, &[]);

        // Assert
        assert!(result.is_ok(), "Should execute successfully with no-edit");

        // Verify access was recorded
        let updated_bookmark = repository.get_by_id(1).unwrap().unwrap();
        assert_eq!(
            updated_bookmark.access_count, 1,
            "Access count should be incremented"
        );
    }

    #[test]
    fn given_non_shell_bookmark_when_execute_default_action_with_no_edit_then_uses_normal_resolver()
    {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();

        let repository = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let bookmark = create_test_bookmark_without_shell_tag();

        // Add bookmark to repository for access recording
        let mut bookmark_copy = bookmark.clone();
        repository.add(&mut bookmark_copy).unwrap();
        let bookmark_id = bookmark_copy.id.unwrap();
        let stored_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();

        // Act
        let result = service.execute_default_action_with_options(&stored_bookmark, true, &[]);

        // Assert
        assert!(result.is_ok(), "Should execute successfully");

        // Verify access was recorded
        let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
        assert_eq!(
            updated_bookmark.access_count, 1,
            "Access count should be incremented"
        );
    }

    #[test]
    fn given_shell_bookmark_when_execute_default_action_with_no_edit_false_then_uses_normal_resolver(
    ) {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();

        let repository = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let bookmark = create_test_bookmark_with_shell_tag();

        // Add bookmark to repository for access recording
        let mut bookmark_copy = bookmark.clone();
        repository.add(&mut bookmark_copy).unwrap();
        let stored_bookmark = repository.get_by_id(1).unwrap().unwrap();

        // Act
        let result = service.execute_default_action_with_options(&stored_bookmark, false, &[]);

        // Assert
        assert!(
            result.is_ok(),
            "Should execute successfully without no-edit"
        );

        // Verify access was recorded
        let updated_bookmark = repository.get_by_id(1).unwrap().unwrap();
        assert_eq!(
            updated_bookmark.access_count, 1,
            "Access count should be incremented"
        );
    }

    #[test]
    fn given_bookmark_without_id_when_execute_default_action_with_options_then_still_executes() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();

        let repository = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let mut bookmark = create_test_bookmark_with_shell_tag();
        bookmark.id = None; // Remove ID to test the case where access recording is skipped

        // Act
        let result = service.execute_default_action_with_options(&bookmark, true, &[]);

        // Assert
        assert!(
            result.is_ok(),
            "Should execute successfully even without bookmark ID"
        );
    }
}
