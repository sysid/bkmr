use bkmr::app_state::AppState;
use bkmr::application::error::ApplicationResult;
use bkmr::application::services::factory;
use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
use bkmr::util::testing::{init_test_env, EnvGuard};
use serial_test::serial;
use std::path::Path;

#[test]
#[serial]
fn given_valid_ndjson_file_when_load_json_bookmarks_then_adds_bookmarks_to_database(
) -> ApplicationResult<()> {
    // Arrange
    let _ = init_test_env();
    let _guard = EnvGuard::new();

    // Clean database for the test
    let app_state = AppState::read_global();
    let repository = SqliteBookmarkRepository::from_url(&app_state.settings.db_url)
        .expect("Could not load bookmarks database");
    repository
        .empty_bookmark_table()
        .expect("Failed to empty bookmark table");

    // Create service with real dependencies
    let bookmark_service = factory::create_bookmark_service();

    // Path to test file
    let test_file_path = Path::new("tests/resources/bookmarks.ndjson")
        .to_str()
        .unwrap()
        .to_string();

    // Act
    let processed_count = bookmark_service.load_json_bookmarks(&test_file_path, false)?;

    // Assert
    // Verify that the correct number of bookmarks were processed
    assert_eq!(
        processed_count, 2,
        "Should have processed 2 bookmarks from the file"
    );

    // Get all bookmarks from the database
    let bookmarks = repository.get_all()?;
    assert_eq!(bookmarks.len(), 2, "Database should contain 2 bookmarks");

    // Verify the first bookmark was added correctly
    let first_bookmark = bookmarks
        .iter()
        .find(|b| b.title == "linear_programming.md")
        .expect("First bookmark not found");

    assert_eq!(first_bookmark.url, "$VIMWIKI_PATH/linear_programming.md:0");
    assert!(
        first_bookmark
            .tags
            .iter()
            .any(|t| t.value() == "_imported_"),
        "First bookmark should have the '_imported_' tag"
    );

    // Verify the second bookmark was added correctly
    let second_bookmark = bookmarks
        .iter()
        .find(|b| b.title == "personal_intro.md")
        .expect("Second bookmark not found");

    assert_eq!(second_bookmark.url, "$VIMWIKI_PATH/personal_intro.md:0");
    assert!(
        second_bookmark
            .tags
            .iter()
            .any(|t| t.value() == "_imported_"),
        "Second bookmark should have the '_imported_' tag"
    );

    Ok(())
}
