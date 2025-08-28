use bkmr::application::error::ApplicationResult;
use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::util::test_service_container::TestServiceContainer;
use std::path::Path;

#[test]
fn given_valid_ndjson_file_when_load_json_bookmarks_then_adds_bookmarks_to_database(
) -> ApplicationResult<()> {
    // Arrange
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;
    
    // Get initial bookmark count (shared database has baseline data)
    let initial_bookmarks = test_container.bookmark_repository.get_all()?;
    let initial_count = initial_bookmarks.len();

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
    let bookmarks = test_container.bookmark_repository.get_all()?;
    assert_eq!(bookmarks.len(), initial_count + 2, "Database should contain {} bookmarks (initial {} + 2 added)", initial_count + 2, initial_count);

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
