use assert_cmd::Command;
use bkmr::util::test_service_container::TestServiceContainer;
use bkmr::domain::tag::Tag;
use bkmr::util::testing::{init_test_env, EnvGuard};
use std::collections::HashSet;

#[test]
fn test_search_command_with_tags() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    // Add test bookmarks with unique tags to the database
    let test_container = TestServiceContainer::new();
    let bookmark_service = test_container.bookmark_service;

    let mut tag_set = HashSet::new();
    tag_set.insert(Tag::new("unique_test_tag").unwrap());
    tag_set.insert(Tag::new("cli_search_test").unwrap());

    // Add test bookmarks (the test expects specific IDs but we'll work with what we get)
    let bookmark1 = bookmark_service
        .add_bookmark(
            "https://example1.com",
            Some("Test Bookmark 1"),
            None,
            Some(&tag_set),
            false,
        )
        .unwrap();

    let bookmark2 = bookmark_service
        .add_bookmark(
            "https://example2.com",
            Some("Test Bookmark 2"),
            None,
            Some(&tag_set),
            false,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("bkmr").expect("Failed to create command");

    // Execute the search command with tag filtering
    let result = cmd
        .arg("search")
        .arg("--tags")
        .arg("unique_test_tag")
        .arg("--np") // non-interactive mode
        .arg("--limit") // non-interactive mode
        .arg("4") // non-interactive mode
        .assert()
        .success();

    // Verify the output contains expected bookmarks with tag "unique_test_tag"
    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show bookmarks found
    assert!(
        stderr.contains("bookmarks"),
        "Should show number of bookmarks found"
    );
    assert!(
        stderr.contains("unique_test_tag"),
        "Should have found bookmark with unique test tag"
    );

    // In non-interactive mode, the output should contain bookmark IDs
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "Should return at least one bookmark ID");

    // Verify that the returned IDs include our test bookmarks
    let output_line = lines[0];
    let returned_ids: Vec<&str> = output_line.split(',').collect();

    // Check that our bookmarks are in the results
    let bookmark1_id = bookmark1.id.unwrap().to_string();
    let bookmark2_id = bookmark2.id.unwrap().to_string();

    assert!(
        returned_ids.contains(&bookmark1_id.as_str())
            || returned_ids.contains(&bookmark2_id.as_str()),
        "Should return at least one of our test bookmark IDs, got: {}",
        output_line
    );
}
