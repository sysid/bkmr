use bkmr::config::{create_file_path_with_base, has_base_path, resolve_file_path, Settings};
use std::collections::HashMap;

#[test]
fn test_base_path_configuration() {
    // Test base path configuration structure
    let mut base_paths = HashMap::new();
    base_paths.insert("SCRIPTS_HOME".to_string(), "$HOME/scripts".to_string());
    base_paths.insert("DOCS_HOME".to_string(), "/home/user/documents".to_string());

    let settings = Settings {
        db_url: "test.db".to_string(),
        fzf_opts: Default::default(),
        shell_opts: Default::default(),
        base_paths,
        config_source: Default::default(),
    };

    // Test has_base_path function
    assert!(has_base_path(&settings, "SCRIPTS_HOME"));
    assert!(has_base_path(&settings, "DOCS_HOME"));
    assert!(!has_base_path(&settings, "NONEXISTENT"));

    // Test create_file_path_with_base function
    let relative_path = create_file_path_with_base("SCRIPTS_HOME", "backup/daily.sh");
    assert_eq!(relative_path, "$SCRIPTS_HOME/backup/daily.sh");

    // Test resolve_file_path function
    let resolved = resolve_file_path(&settings, "$DOCS_HOME/readme.md");
    assert_eq!(resolved, "/home/user/documents/readme.md");
}

#[test]
fn test_base_path_usage_examples() {
    // Example: SCRIPTS_HOME = "$HOME/scripts"
    // User runs: bkmr import-files backup --base-path SCRIPTS_HOME
    // This looks for files in: $HOME/scripts + backup = $HOME/scripts/backup/*
    // And stores them as: $SCRIPTS_HOME/backup/script.sh

    let mut base_paths = HashMap::new();
    base_paths.insert("SCRIPTS_HOME".to_string(), "$HOME/scripts".to_string());

    let settings = Settings {
        db_url: "test.db".to_string(),
        fzf_opts: Default::default(),
        shell_opts: Default::default(),
        base_paths,
        config_source: Default::default(),
    };

    // Simulate how the system resolves paths
    let _ = resolve_file_path(&settings, "$HOME/scripts");
    let _ = "backup"; // User provides relative path

    // The FileImportRepository would combine these to scan files
    // Full scan path would be: base_expanded + "/" + user_provided_path

    // When storing, a file like backup/daily.sh becomes $SCRIPTS_HOME/backup/daily.sh
    let stored_path = create_file_path_with_base("SCRIPTS_HOME", "backup/daily.sh");
    assert_eq!(stored_path, "$SCRIPTS_HOME/backup/daily.sh");
}

#[test]
fn test_environment_variable_expansion() {
    let mut base_paths = HashMap::new();
    base_paths.insert("TEST_HOME".to_string(), "$HOME/test".to_string());

    let settings = Settings {
        db_url: "test.db".to_string(),
        fzf_opts: Default::default(),
        shell_opts: Default::default(),
        base_paths,
        config_source: Default::default(),
    };

    let resolved = resolve_file_path(&settings, "$TEST_HOME/file.txt");
    // Should expand $HOME environment variable
    assert!(resolved.contains("/test/file.txt"));
    assert!(!resolved.contains("$HOME"));
    assert!(!resolved.contains("$TEST_HOME"));
}
