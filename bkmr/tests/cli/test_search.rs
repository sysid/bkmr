use assert_cmd::cargo::cargo_bin_cmd;
use bkmr::domain::tag::Tag;
use bkmr::util::test_service_container::TestServiceContainer;
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
            Some(true),
            None,
        )
        .unwrap();

    let bookmark2 = bookmark_service
        .add_bookmark(
            "https://example2.com",
            Some("Test Bookmark 2"),
            None,
            Some(&tag_set),
            false,
            Some(true),
            None,
        )
        .unwrap();

    let mut cmd = cargo_bin_cmd!("bkmr");

    // Set environment variable to use the same test database
    cmd.env("BKMR_DB_URL", "../db/bkmr.db");

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

#[test]
fn given_no_matching_bookmarks_when_hsearch_json_then_stdout_is_empty_array() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();

    let mut cmd = cargo_bin_cmd!("bkmr");
    cmd.env("BKMR_DB_URL", "../db/bkmr.db");

    // Exact mode keeps the embedding model out of the test; the term matches nothing.
    let result = cmd
        .args(["hsearch", "--json", "--np", "--mode", "exact"])
        .arg("zzqxnomatchterm")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&result.get_output().stdout);
    assert_eq!(
        stdout.trim(),
        "[]",
        "JSON consumers need valid JSON on zero hits"
    );
}

/// Titles that `hsearch --mode exact` returns. Exact mode needs no embedding model.
fn hsearch_exact_titles(query: &str) -> Vec<String> {
    let result = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", "../db/bkmr.db")
        .args(["hsearch", "--json", "--np", "--mode", "exact", query])
        .assert()
        .success();
    let hits: Vec<serde_json::Value> = serde_json::from_slice(&result.get_output().stdout).unwrap();
    hits.iter()
        .map(|hit| hit["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn given_scattered_inflected_terms_when_hsearch_exact_then_matches() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let bookmark_service = TestServiceContainer::new().bookmark_service;
    bookmark_service
        .add_bookmark(
            "https://example.com/mode-doc",
            Some("Zqrunning pods on the zqcluster"),
            None,
            None,
            false,
            Some(false),
            None,
        )
        .unwrap();

    // Different case, different word order, different word ending, words not adjacent.
    assert_eq!(
        hsearch_exact_titles("ZQCLUSTER zqruns"),
        vec!["Zqrunning pods on the zqcluster"]
    );
}

#[test]
fn given_one_term_missing_when_hsearch_exact_then_no_match() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let bookmark_service = TestServiceContainer::new().bookmark_service;
    bookmark_service
        .add_bookmark(
            "https://example.com/mode-doc",
            Some("Zqrunning pods on the zqcluster"),
            None,
            None,
            false,
            Some(false),
            None,
        )
        .unwrap();

    assert!(hsearch_exact_titles("zqcluster zqabsent").is_empty());
}

#[test]
fn given_term_with_punctuation_when_hsearch_exact_then_punctuation_is_ignored() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let bookmark_service = TestServiceContainer::new().bookmark_service;
    bookmark_service
        .add_bookmark(
            "https://example.com/mode-doc",
            Some("Zqlang basics"),
            None,
            None,
            false,
            Some(false),
            None,
        )
        .unwrap();

    // The title has no `++`: the term is safe to type, but not matched character by character.
    assert_eq!(hsearch_exact_titles("zqlang++"), vec!["Zqlang basics"]);
}
