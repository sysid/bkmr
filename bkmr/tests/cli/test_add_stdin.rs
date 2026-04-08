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
fn given_stdin_content_when_add_then_stores_content_in_url_column() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    let test_content = "echo 'Hello from stdin test!'";

    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("test").unwrap());
    tag_set.insert(Tag::new("stdin").unwrap());
    tag_set.insert(Tag::new("_shell_").unwrap());

    let bookmark = bookmark_service
        .add_bookmark(
            test_content,
            Some("test_stdin_command"),
            None,
            Some(&tag_set),
            false,
            true,
        )
        .unwrap();

    assert!(bookmark.id.is_some(), "Bookmark should have an ID");
    assert_eq!(
        bookmark.url, test_content,
        "URL should contain the stdin content"
    );
    assert_eq!(bookmark.title, "test_stdin_command");
    assert_eq!(bookmark.description, "");
    assert!(bookmark.tags.contains(&Tag::new("_shell_").unwrap()));
    assert!(bookmark.tags.contains(&Tag::new("test").unwrap()));
    assert!(bookmark.tags.contains(&Tag::new("stdin").unwrap()));

    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(stored_bookmark.url, test_content);
}

#[test]
fn given_stdin_with_shell_type_when_add_then_creates_shell_bookmark() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    let test_script = "#!/bin/bash\necho 'Complex shell script'\nls -la";
    let title = "complex_shell_script";

    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("_shell_").unwrap());
    tag_set.insert(Tag::new("automation").unwrap());
    tag_set.insert(Tag::new("test").unwrap());

    let bookmark = bookmark_service
        .add_bookmark(test_script, Some(title), None, Some(&tag_set), false, true)
        .unwrap();

    assert!(bookmark.id.is_some());
    assert_eq!(bookmark.url, test_script);
    assert_eq!(bookmark.title, title);
    assert_eq!(bookmark.description, "");
    assert!(bookmark.tags.contains(&Tag::new("_shell_").unwrap()));

    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(stored_bookmark.url, test_script);
    assert!(stored_bookmark.tags.contains(&Tag::new("_shell_").unwrap()));
}

#[test]
fn given_stdin_with_multiline_content_when_add_then_preserves_formatting() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    let multiline_content = "#!/bin/bash\n\n# This is a comment\necho 'Line 1'\necho 'Line 2'\n\n# Another comment\necho 'Line 3'";
    let title = "multiline_script";

    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("_shell_").unwrap());
    tag_set.insert(Tag::new("multiline").unwrap());

    let bookmark = bookmark_service
        .add_bookmark(multiline_content, Some(title), None, Some(&tag_set), false, true)
        .unwrap();

    assert_eq!(bookmark.url, multiline_content);

    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(stored_bookmark.url, multiline_content);
    assert!(stored_bookmark.url.contains('\n'));
    assert!(stored_bookmark.url.contains("# This is a comment"));
}
