use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use bkmr::util::test_service_container::TestServiceContainer;
use bkmr::util::testing::{init_test_env, EnvGuard};
use std::collections::HashSet;

fn create_test_repository() -> SqliteBookmarkRepository {
    let repository = bkmr::util::testing::setup_test_db();
    repository
        .empty_bookmark_table()
        .expect("Could not empty bookmark table");
    repository
}

#[test]
fn given_shell_bookmark_when_open_with_no_edit_then_executes_without_interaction() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;
    let action_service = test_container.action_service;

    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("_shell_").unwrap());
    tag_set.insert(Tag::new("test").unwrap());

    let test_script = "echo 'Test script executed'";
    let bookmark = bookmark_service
        .add_bookmark(
            test_script,
            Some("test_no_edit"),
            None,
            Some(&tag_set),
            false,
        )
        .unwrap();

    let bookmark_id = bookmark.id.unwrap();

    let stored_bookmark = bookmark_service.get_bookmark(bookmark_id).unwrap().unwrap();
    let result = action_service.execute_default_action_with_options(&stored_bookmark, true, &[]);

    assert!(
        result.is_ok(),
        "Should execute successfully with no-edit flag"
    );

    let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
    assert_eq!(
        updated_bookmark.access_count, 1,
        "Access count should be incremented"
    );
}

#[test]
fn given_url_bookmark_when_open_with_no_edit_then_executes_normally() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;
    let action_service = test_container.action_service;

    let tag_set = HashSet::new();

    let test_url = "https://example.com";
    let bookmark = bookmark_service
        .add_bookmark(
            test_url,
            Some("test_url_no_edit"),
            None,
            Some(&tag_set),
            false,
        )
        .unwrap();

    let bookmark_id = bookmark.id.unwrap();

    let stored_bookmark = bookmark_service.get_bookmark(bookmark_id).unwrap().unwrap();
    let result = action_service.execute_default_action_with_options(&stored_bookmark, true, &[]);

    assert!(
        result.is_ok(),
        "Should execute successfully even for non-shell bookmarks"
    );

    let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
    assert_eq!(
        updated_bookmark.access_count, 1,
        "Access count should be incremented"
    );
}
