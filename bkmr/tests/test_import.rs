// tests/test_import.rs

use bkmr::domain::repositories::repository::BookmarkRepository;
use bkmr::util::test_context::TestContext;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_import_files_with_yaml_frontmatter() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Test importing files with YAML frontmatter
    let test_dir = "tests/resources/import_test";
    let paths = vec![test_dir.to_string()];

    // First import (should add files)
    let result = service.import_files(&paths, false, false, false, false, None, true);
    if let Err(e) = &result {
        eprintln!("Import failed: {:?}", e);
    }
    assert!(result.is_ok(), "Import should succeed");

    let (added, updated, deleted) = result.unwrap();
    assert!(added > 0, "Should add files");
    assert_eq!(updated, 0, "Should not update files on first import");
    assert_eq!(deleted, 0, "Should not delete files on first import");
}

#[test]
fn test_import_files_duplicate_name_without_update() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Create a temporary file with duplicate name
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("duplicate.sh");

    // Create first file
    fs::write(
        &file_path,
        r#"---
name: backup-database
tags: test
---
#!/bin/bash
echo "duplicate test"
"#,
    )
    .unwrap();

    let paths = vec![temp_dir.path().to_string_lossy().to_string()];

    // First import should succeed
    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(result.is_ok(), "First import should succeed");

    // Create second file with same name
    let file_path2 = temp_dir.path().join("duplicate2.sh");
    fs::write(
        &file_path2,
        r#"---
name: backup-database
tags: test2
---
#!/bin/bash
echo "another duplicate"
"#,
    )
    .unwrap();

    // Second import without --update should fail with DuplicateName error
    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(result.is_err(), "Import should fail due to duplicate name");

    if let Err(e) = result {
        assert!(
            e.to_string().contains("Duplicate name"),
            "Should be duplicate name error"
        );
    }
}

#[test]
fn test_import_files_update_existing() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Create a temporary file
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.sh");

    // Create initial file
    fs::write(
        &file_path,
        r#"---
name: test-script
tags: test
---
#!/bin/bash
echo "version 1"
"#,
    )
    .unwrap();

    let paths = vec![temp_dir.path().to_string_lossy().to_string()];

    // First import
    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(result.is_ok(), "First import should succeed");
    let (added, _, _) = result.unwrap();
    assert_eq!(added, 1, "Should add one file");

    // Update file content
    fs::write(
        &file_path,
        r#"---
name: test-script
tags: test, updated
---
#!/bin/bash
echo "version 2"
"#,
    )
    .unwrap();

    // Import with update flag
    let result = service.import_files(&paths, true, false, false, false, None, true);
    assert!(result.is_ok(), "Update import should succeed");
    let (added, updated, _) = result.unwrap();
    assert_eq!(added, 0, "Should not add new files");
    assert_eq!(updated, 1, "Should update one file");
}

#[test]
fn test_import_files_dry_run() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    let test_dir = "tests/resources/import_test";
    let paths = vec![test_dir.to_string()];

    // Dry run should not modify database
    let result = service.import_files(&paths, false, false, true, false, None, true);
    assert!(result.is_ok(), "Dry run should succeed");

    let (added, _updated, _deleted) = result.unwrap();
    // In dry run, it should report what would be done but not actually do it
    assert!(added > 0, "Should report files that would be added");

    // Verify no bookmarks were actually added
    let all_bookmarks = service.get_all_bookmarks(None, None).unwrap();
    let file_bookmarks = all_bookmarks
        .iter()
        .filter(|b| b.file_path.is_some())
        .count();
    assert_eq!(file_bookmarks, 0, "No bookmarks should be added in dry run");
}

#[test]
fn test_import_files_hash_comments() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Create a temporary file with hash-style frontmatter
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("hash_test.sh");

    fs::write(
        &file_path,
        r#"#!/bin/bash
# name: hash-style-script  
# tags: test, shell
# type: _shell_

echo "This uses hash-style frontmatter"
ls -la
"#,
    )
    .unwrap();

    let paths = vec![temp_dir.path().to_string_lossy().to_string()];

    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(result.is_ok(), "Import with hash comments should succeed");

    let (added, _, _) = result.unwrap();
    assert_eq!(added, 1, "Should add one file");
}

#[test]
fn test_import_files_missing_name_field() {
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Create a file without required name field
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("no_name.sh");

    fs::write(
        &file_path,
        r#"---
tags: test
---
#!/bin/bash
echo "missing name field"
"#,
    )
    .unwrap();

    let paths = vec![temp_dir.path().to_string_lossy().to_string()];

    // Should succeed but skip files without names (warnings should be logged)
    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(
        result.is_ok(),
        "Import should succeed but skip invalid files"
    );

    let (added, _, _) = result.unwrap();
    assert_eq!(added, 0, "Should not add files without name field");
}

#[test]
fn given_file_with_snip_type_when_import_then_bookmark_has_snip_tag() {
    use bkmr::domain::tag::Tag;

    let ctx = TestContext::new();
    let service = ctx.bookmark_service();

    // Create a temporary file with type: _snip_ in frontmatter
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("snippet.md");

    fs::write(
        &file_path,
        r#"---
name: my-snippet
tags: test
type: _snip_
---
This is snippet content that should be tagged as _snip_, not _shell_.
"#,
    )
    .unwrap();

    let paths = vec![temp_dir.path().to_string_lossy().to_string()];

    // Import the file
    let result = service.import_files(&paths, false, false, false, false, None, true);
    assert!(result.is_ok(), "Import should succeed");

    let (added, _, _) = result.unwrap();
    assert_eq!(added, 1, "Should add one file");

    // Verify the bookmark has _snip_ tag, not _shell_
    let all_bookmarks = service.get_all_bookmarks(None, None).unwrap();
    let bookmark = all_bookmarks
        .iter()
        .find(|b| b.title == "my-snippet")
        .expect("Should find the imported bookmark");

    assert!(
        bookmark.tags.contains(&Tag::new("_snip_").unwrap()),
        "Bookmark should have _snip_ tag from frontmatter type"
    );
    assert!(
        !bookmark.tags.contains(&Tag::new("_shell_").unwrap()),
        "Bookmark should NOT have _shell_ tag"
    );
}

#[test]
fn given_no_embed_when_importing_and_updating_file_then_stores_no_content_hash() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("no_hash.sh");
    fs::write(&file, "#!/bin/bash\n# name: no-hash\n# type: _shell_\necho one\n").unwrap();
    let paths = vec![temp_dir.path().to_string_lossy().to_string()];
    let stored = || {
        ctx.repository()
            .get_all()
            .unwrap()
            .into_iter()
            .find(|b| b.title == "no-hash")
            .expect("file imported")
    };

    // Act + Assert: a non-embeddable bookmark has no hash after the first import
    service
        .import_files(&paths, false, false, false, false, None, false)
        .unwrap();
    assert!(!stored().embeddable);
    assert_eq!(stored().content_hash, None);

    // Act + Assert: nor after the file changed
    fs::write(&file, "#!/bin/bash\n# name: no-hash\n# type: _shell_\necho two\n").unwrap();
    service
        .import_files(&paths, true, false, false, false, None, false)
        .unwrap();
    assert!(stored().url.contains("echo two"));
    assert_eq!(stored().content_hash, None);
}
