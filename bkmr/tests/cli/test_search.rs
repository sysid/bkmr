use assert_cmd::Command;
use serial_test::serial;
use bkmr::util::testing::{init_test_env, EnvGuard};

#[test]
#[serial]
fn test_search_command_with_tags() {
    // Setup test environment with the example database
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    
    let mut cmd = Command::cargo_bin("bkmr").expect("Failed to create command");
    
    // Execute the search command with tag filtering
    let result = cmd
        .arg("search")
        .arg("--tags")
        .arg("aaa")
        .arg("--np")  // non-interactive mode
        .arg("--limit")  // non-interactive mode
        .arg("4")  // non-interactive mode
        .assert()
        .success();
    
    // Verify the output contains expected bookmarks with tag "aaa"
    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // The example database has bookmarks with "aaa" tag
    assert!(stderr.contains("bookmarks"), "Should show number of bookmarks found");
    
    // In non-interactive mode, the output should contain bookmark IDs
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "Should return at least one bookmark ID");
    
    assert_eq!(lines[0], "3,4,5,6", "Should return bookmark IDs: 3,4,5,6");
}