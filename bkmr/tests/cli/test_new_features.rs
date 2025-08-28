use bkmr::util::test_service_container::TestServiceContainer;
use bkmr::cli::args::{Cli, Commands};
use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use bkmr::util::testing::{init_test_env, EnvGuard};
use std::collections::HashSet;
use std::io::Write;
use tempfile::NamedTempFile;

fn create_test_repository() -> SqliteBookmarkRepository {
    // Use test infrastructure instead of global state
    let repository = bkmr::util::testing::setup_test_db();

    // Clean the database for test isolation
    repository
        .empty_bookmark_table()
        .expect("Could not empty bookmark table");

    repository
}

#[test]
fn given_stdin_content_when_add_command_with_stdin_flag_then_stores_content_in_url_column() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    // Create a temporary file to simulate stdin
    let mut temp_file = NamedTempFile::new().unwrap();
    let test_content = "echo 'Hello from stdin test!'";
    temp_file.write_all(test_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Simulate the CLI command structure (not executed in this test)
    let _cli = Cli {
        name: None,
        config: None,
        debug: 0,
        openai: false,
        no_color: false,
        generate_config: false,
        command: Some(Commands::Add {
            url: None,
            tags: Some("test,stdin".to_string()),
            title: Some("test_stdin_command".to_string()),
            desc: None,
            no_web: true,
            edit: false,
            bookmark_type: "shell".to_string(),
            clone_id: None,
            stdin: true,
        }),
    };

    // Simulate stdin input by setting final_url directly in the test
    // We'll manually test the core logic by calling the bookmark service directly
    let final_content = test_content.to_string();

    // Create the expected tags
    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("test").unwrap());
    tag_set.insert(Tag::new("stdin").unwrap());
    tag_set.insert(Tag::new("_shell_").unwrap());

    // Act - Add the bookmark with stdin content
    let bookmark = bookmark_service
        .add_bookmark(
            &final_content,
            Some("test_stdin_command"),
            None,
            Some(&tag_set),
            false,
        )
        .unwrap();

    // Assert
    assert!(bookmark.id.is_some(), "Bookmark should have an ID");
    assert_eq!(
        bookmark.url, test_content,
        "URL should contain the stdin content"
    );
    assert_eq!(bookmark.title, "test_stdin_command", "Title should match");
    assert_eq!(bookmark.description, "", "Description should be empty");
    assert!(
        bookmark.tags.contains(&Tag::new("_shell_").unwrap()),
        "Should have shell tag"
    );
    assert!(
        bookmark.tags.contains(&Tag::new("test").unwrap()),
        "Should have test tag"
    );
    assert!(
        bookmark.tags.contains(&Tag::new("stdin").unwrap()),
        "Should have stdin tag"
    );

    // Verify it's stored correctly in the database
    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(
        stored_bookmark.url, test_content,
        "Stored URL should contain the stdin content"
    );
}

#[test]
fn given_shell_bookmark_when_open_command_with_no_edit_flag_then_executes_without_interaction() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;
    let action_service = test_container.action_service;

    // Create a shell bookmark
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

    // Simulate the CLI command structure with --no-edit flag (not executed in this test)
    let _cli = Cli {
        name: None,
        config: None,
        debug: 0,
        openai: false,
        no_color: false,
        generate_config: false,
        command: Some(Commands::Open {
            ids: bookmark_id.to_string(),
            no_edit: true,
            file: false,
            script_args: vec![],
        }),
    };

    // Act - Execute the open command with no_edit=true
    let stored_bookmark = bookmark_service.get_bookmark(bookmark_id).unwrap().unwrap();
    let result = action_service.execute_default_action_with_options(&stored_bookmark, true, &[]);

    // Assert
    assert!(
        result.is_ok(),
        "Should execute successfully with no-edit flag"
    );

    // Verify access was recorded
    let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
    assert_eq!(
        updated_bookmark.access_count, 1,
        "Access count should be incremented"
    );
}

#[test]
fn given_non_shell_bookmark_when_open_command_with_no_edit_flag_then_executes_normally() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;
    let action_service = test_container.action_service;

    // Create a non-shell bookmark (URI)
    let tag_set = HashSet::new(); // No shell tag

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

    // Act - Execute the open command with no_edit=true (should not affect non-shell bookmarks)
    let stored_bookmark = bookmark_service.get_bookmark(bookmark_id).unwrap().unwrap();
    let result = action_service.execute_default_action_with_options(&stored_bookmark, true, &[]);

    // Assert
    assert!(
        result.is_ok(),
        "Should execute successfully even for non-shell bookmarks"
    );

    // Verify access was recorded
    let updated_bookmark = repository.get_by_id(bookmark_id).unwrap().unwrap();
    assert_eq!(
        updated_bookmark.access_count, 1,
        "Access count should be incremented"
    );
}

#[test]
fn given_add_command_with_stdin_and_type_shell_when_executed_then_creates_shell_bookmark() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let repository = create_test_repository();
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    let test_script = "#!/bin/bash\necho 'Complex shell script'\nls -la";
    let title = "complex_shell_script";

    // Create the expected tags for shell type
    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("_shell_").unwrap());
    tag_set.insert(Tag::new("automation").unwrap());
    tag_set.insert(Tag::new("test").unwrap());

    // Act - Simulate adding with stdin (manually test the core logic)
    let bookmark = bookmark_service
        .add_bookmark(test_script, Some(title), None, Some(&tag_set), false)
        .unwrap();

    // Assert
    assert!(bookmark.id.is_some(), "Bookmark should have an ID");
    assert_eq!(
        bookmark.url, test_script,
        "URL should contain the script content"
    );
    assert_eq!(bookmark.title, title, "Title should match");
    assert_eq!(
        bookmark.description, "",
        "Description should be empty for stdin input"
    );
    assert!(
        bookmark.tags.contains(&Tag::new("_shell_").unwrap()),
        "Should have shell tag"
    );

    // Verify the bookmark type is correctly identified
    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(
        stored_bookmark.url, test_script,
        "Stored script should match input"
    );
    assert!(
        stored_bookmark.tags.contains(&Tag::new("_shell_").unwrap()),
        "Should be tagged as shell"
    );
}

#[test]
fn given_stdin_with_multiline_content_when_add_command_then_preserves_formatting() {
    // Arrange
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

    // Act
    let bookmark = bookmark_service
        .add_bookmark(multiline_content, Some(title), None, Some(&tag_set), false)
        .unwrap();

    // Assert
    assert_eq!(
        bookmark.url, multiline_content,
        "Multiline content should be preserved exactly"
    );

    // Verify in database
    let stored_bookmark = repository.get_by_id(bookmark.id.unwrap()).unwrap().unwrap();
    assert_eq!(
        stored_bookmark.url, multiline_content,
        "Stored multiline content should match"
    );
    assert!(
        stored_bookmark.url.contains('\n'),
        "Should preserve newlines"
    );
    assert!(
        stored_bookmark.url.contains("# This is a comment"),
        "Should preserve comments"
    );
}
