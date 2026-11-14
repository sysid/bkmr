// src/application/services/action_service.rs
use crate::domain::action_resolver::ActionResolver;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::util::validation::ValidationHelper;
use std::sync::Arc;
use tracing::{debug, instrument, warn};

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

    /// Counts an attempt to deliver this bookmark's content. A failed write only warns:
    /// losing a count must not block the content the user asked for.
    fn record_access(&self, bookmark: &Bookmark);
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
        // Before the action, so that a long-running or killed shell action is still counted
        self.record_access(bookmark);

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
        use crate::domain::action::BookmarkAction;
        use crate::infrastructure::interpolation::minijinja_engine::{
            MiniJinjaEngine, SystemShellExecutor,
        };
        use std::sync::Arc;

        // Before the action, so that a long-running or killed shell action is still counted
        self.record_access(bookmark);

        // Use the resolver for consistent action dispatch, then apply --no-edit override
        if no_edit && bookmark.is_shell() {
            debug!("Executing shell action with no-edit mode");

            // Create a direct (non-interactive) shell action with script arguments
            let shell_executor = Arc::new(SystemShellExecutor::new());
            let template_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
            let interpolation_service = Arc::new(
                crate::application::InterpolationServiceImpl::new(template_engine),
            );
            let shell_action =
                ShellAction::new_direct_with_args(interpolation_service, script_args.to_vec());
            return shell_action.execute(bookmark);
        }

        // Use the normal action resolver
        let action = self.resolver.resolve_action(bookmark);
        debug!("Executing action: {}", action.description());
        action.execute(bookmark)
    }

    fn get_default_action_description(&self, bookmark: &Bookmark) -> &'static str {
        let action = self.resolver.resolve_action(bookmark);
        action.description()
    }

    #[instrument(skip(self, bookmark), level = "trace")]
    fn record_access(&self, bookmark: &Bookmark) {
        if let Some(id) = bookmark.id {
            if let Err(e) = self.repository.record_access(id) {
                warn!("Failed to record access for bookmark {}: {}", id, e);
            }
        }
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
    use tempfile::TempDir;

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

    // A private file DB per test: isolated from the shared test DB, and unlike `:memory:`
    // every pooled connection sees the same database.
    // The returned TempDir must stay alive as long as the repository is used.
    fn create_test_repository() -> (Arc<SqliteBookmarkRepository>, TempDir) {
        let dir = TempDir::new().expect("Could not create temp dir");
        let db_url = dir.path().join("action_service.db");
        let repository = SqliteBookmarkRepository::from_url(db_url.to_str().unwrap())
            .expect("Could not create test repository");

        (Arc::new(repository), dir)
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
            opener: None,
            accessed_at: None,
            verified_at: None,
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
            opener: None,
            accessed_at: None,
            verified_at: None,
        }
    }

    #[test]
    fn given_shell_bookmark_when_execute_default_action_with_no_edit_then_uses_direct_shell_action()
    {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();

        let (repository, _db_dir) = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let bookmark = create_test_bookmark_with_shell_tag();

        // Add bookmark to repository for access recording
        let mut bookmark_copy = bookmark.clone();
        repository.add(&mut bookmark_copy).unwrap();
        let bookmark_id = bookmark_copy.id.unwrap();
        let stored_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
        assert_eq!(
            stored_bookmark.url, "echo 'test script'",
            "Test must act on the shell bookmark it added"
        );

        // Act
        let result = service.execute_default_action_with_options(&stored_bookmark, true, &[]);

        // Assert
        assert!(result.is_ok(), "Should execute successfully with no-edit");

        // Verify access was recorded
        let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
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

        let (repository, _db_dir) = create_test_repository();
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

        let (repository, _db_dir) = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(Arc::clone(&mock_action)));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));

        let bookmark = create_test_bookmark_with_shell_tag();

        // Add bookmark to repository for access recording
        let mut bookmark_copy = bookmark.clone();
        repository.add(&mut bookmark_copy).unwrap();
        let bookmark_id = bookmark_copy.id.unwrap();
        let stored_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
        assert_eq!(
            stored_bookmark.url, "echo 'test script'",
            "Test must act on the shell bookmark it added"
        );

        // Act
        let result = service.execute_default_action_with_options(&stored_bookmark, false, &[]);

        // Assert
        assert!(
            result.is_ok(),
            "Should execute successfully without no-edit"
        );

        // Verify access was recorded
        let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
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

        let (repository, _db_dir) = create_test_repository();
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

    fn make_access_writes_fail(repository: &SqliteBookmarkRepository) {
        use diesel::RunQueryDsl;

        let mut conn = repository.get_connection().unwrap();
        diesel::sql_query(
            "CREATE TRIGGER fail_access_write BEFORE UPDATE ON bookmarks \
             BEGIN SELECT RAISE(ABORT, 'write refused'); END",
        )
        .execute(&mut conn)
        .unwrap();
    }

    #[test]
    fn given_failing_access_write_when_execute_default_action_then_action_still_runs() {
        let (repository, _db_dir) = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(mock_action));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));
        let mut bookmark = create_test_bookmark_without_shell_tag();
        repository.add(&mut bookmark).unwrap();
        make_access_writes_fail(&repository);

        let result = service.execute_default_action(&bookmark);

        assert!(result.is_ok(), "got {:?}", result);
    }

    #[test]
    fn given_failing_access_write_when_execute_with_options_then_action_still_runs() {
        let (repository, _db_dir) = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(mock_action));
        let service = ActionServiceImpl::new(resolver, Arc::clone(&repository));
        let mut bookmark = create_test_bookmark_without_shell_tag();
        repository.add(&mut bookmark).unwrap();
        make_access_writes_fail(&repository);

        let result = service.execute_default_action_with_options(&bookmark, false, &[]);

        assert!(result.is_ok(), "got {:?}", result);
    }

    #[test]
    fn given_concurrent_actions_when_executed_then_every_access_is_counted() {
        const THREADS: usize = 4;
        const ACCESSES_PER_THREAD: usize = 25;

        let (repository, _db_dir) = create_test_repository();
        let mock_action = Arc::new(MockAction::new("Mock action"));
        let resolver = Arc::new(MockActionResolver::new(mock_action));
        let service = Arc::new(ActionServiceImpl::new(resolver, Arc::clone(&repository)));
        let mut bookmark = create_test_bookmark_without_shell_tag();
        repository.add(&mut bookmark).unwrap();

        let workers: Vec<_> = (0..THREADS)
            .map(|_| {
                let service = Arc::clone(&service);
                let bookmark = bookmark.clone();
                std::thread::spawn(move || {
                    for _ in 0..ACCESSES_PER_THREAD {
                        service.execute_default_action(&bookmark).unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }

        let stored = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
        assert_eq!(stored.access_count as usize, THREADS * ACCESSES_PER_THREAD);
    }
}
