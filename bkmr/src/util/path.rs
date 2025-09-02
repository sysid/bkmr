// src/util/path.rs
use regex::Regex;
use std::path::{Path, PathBuf};

/// resolves existing path and follows symlinks, returns None if path does not exist
/// also removes suffix like ":1" or ":0" from the path if present
pub fn abspath(p: &str) -> Option<String> {
    // Compile a regex to find a suffix pattern like ":<integer>"
    let regex = Regex::new(":\\d+$").unwrap();

    // Remove the suffix if present
    let p_without_suffix = regex.replace(p, "");

    let expanded_path = shellexpand::full(&p_without_suffix).ok()?;
    let path = Path::new(expanded_path.as_ref());

    let abs_path = path.canonicalize().ok()?;
    abs_path.to_str().map(|s| s.to_string())
}

/// Prepare test directory with test data and return path
pub fn temp_dir() -> PathBuf {
    let tempdir = tempfile::tempdir().unwrap();
    let options = fs_extra::dir::CopyOptions::new(); //Initialize default values for CopyOptions
    fs_extra::copy_items(
        &[
            "tests/resources/schema_v1_migration_test.db",
            "tests/resources/schema_v2_with_embeddings.db",
        ],
        tempdir.path(),
        &options,
    )
    .expect("Failed to copy test project directory");

    tempdir.keep()
}

/// Checks if the given string is likely a file path rather than a URL or direct markdown
pub fn is_file_path(content: &str) -> bool {
    // First trim any whitespace
    let trimmed = content.trim();

    // Exclude obvious URLs
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
        || trimmed.starts_with("file://")
    {
        return false;
    }

    // Check for obvious markdown content (starts with markdown syntax)
    if trimmed.starts_with('#')
        || trimmed.starts_with('-')
        || trimmed.starts_with('*')
        || trimmed.starts_with('>')
    {
        return false;
    }

    // Check for file path indicators
    if trimmed.starts_with('/')
        || trimmed.starts_with('~')
        || trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed.starts_with("$HOME")
        || trimmed.starts_with("$")
    {
        return true;
    }

    // Check for file extension
    if trimmed.contains('.')
        && !trimmed.contains(' ')
        && !trimmed.contains('\n')
        && Path::new(trimmed).extension().is_some()
    {
        // Check for known markdown file extensions
        let extension = Path::new(trimmed)
            .extension()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        if ["md", "markdown", "txt", "text"].contains(&extension.as_str()) {
            return true;
        }

        // If it has forward slashes, it's likely a path
        if trimmed.contains('/') {
            return true;
        }
    }

    false
}

/// Expand environment variables and tilde in path without requiring the path to exist
/// This is useful for database URLs and other paths that may not exist yet
pub fn expand_path(path: &str) -> String {
    match shellexpand::full(path) {
        Ok(expanded) => expanded.to_string(),
        Err(_) => path.to_string(),
    }
}

/// Extract filename from: $HOME/bla/file.md:0
pub fn extract_filename(input: &str) -> String {
    // Attempt to split the input string by ':' to handle potential line indicators
    let parts: Vec<&str> = input.split(':').collect();
    let path_str = parts[0]; // The path part of the input

    // Use the Path type to manipulate file paths
    let path = Path::new(path_str);

    // Extract the filename, if it exists, and convert it to a String
    path.file_name().map_or(input.to_string(), |filename| {
        filename.to_string_lossy().to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn given_path_with_suffix_when_abspath_then_removes_suffix_and_canonicalizes() {
        // Create a temporary file.
        let temp_dir = env::temp_dir();
        let file_path = temp_dir.join("test_file.txt");
        let mut file = File::create(&file_path).expect("Failed to create temporary file");
        writeln!(file, "Hello world").expect("Failed to write to temporary file");

        // Append a suffix like ":1"
        let file_str = file_path.to_str().unwrap();
        let input = format!("{}:1", file_str);
        let abs = abspath(&input);

        // The expected result is the canonicalized version of the original file path.
        let canon = fs::canonicalize(&file_path).expect("Failed to canonicalize path");
        assert_eq!(abs, Some(canon.to_str().unwrap().to_string()));

        // Cleanup
        fs::remove_file(file_path).expect("Failed to remove temporary file");
    }

    #[test]
    fn given_path_with_suffix_when_extract_filename_then_returns_filename_without_suffix() {
        let input = "/home/user/docs/report.pdf:0";
        let filename = extract_filename(input);
        assert_eq!(filename, "report.pdf");
    }

    #[test]
    fn given_various_strings_when_is_file_path_then_correctly_identifies_paths() {
        // Test cases
        assert!(is_file_path("/absolute/path/file.md"));
        assert!(is_file_path("~/relative/to/home.md"));
        assert!(is_file_path("./relative/path.md"));
        assert!(is_file_path("../parent/path.md"));
        assert!(is_file_path("$HOME/documents/file.md"));
        assert!(is_file_path("file.md")); // Simple filename with extension

        // Not file paths
        assert!(!is_file_path("# Markdown heading"));
        assert!(!is_file_path("This is a paragraph of text"));
        assert!(!is_file_path("https://example.com"));
        assert!(!is_file_path(" "));
        assert!(!is_file_path(""));
    }

    #[test]
    fn given_path_with_home_var_when_expand_path_then_expands_correctly() {
        // Test $HOME expansion
        let home_path = "$HOME/.config/bkmr/bkmr.db";
        let expanded = expand_path(home_path);
        
        // Should not contain literal $HOME anymore
        assert!(!expanded.contains("$HOME"));
        
        // Should contain the actual home directory path
        if let Ok(home) = env::var("HOME") {
            assert!(expanded.starts_with(&home));
            assert!(expanded.ends_with("/.config/bkmr/bkmr.db"));
        }
    }

    #[test]
    fn given_path_with_tilde_when_expand_path_then_expands_correctly() {
        // Test tilde expansion
        let tilde_path = "~/.config/bkmr/bkmr.db";
        let expanded = expand_path(tilde_path);
        
        // Should not contain literal tilde anymore
        assert!(!expanded.starts_with("~"));
        
        // Should be an absolute path
        assert!(expanded.starts_with("/"));
        assert!(expanded.ends_with("/.config/bkmr/bkmr.db"));
    }

    #[test]
    fn given_absolute_path_when_expand_path_then_returns_unchanged() {
        let absolute_path = "/absolute/path/to/database.db";
        let expanded = expand_path(absolute_path);
        
        assert_eq!(expanded, absolute_path);
    }

    #[test]
    fn given_relative_path_when_expand_path_then_returns_unchanged() {
        let relative_path = "./relative/database.db";
        let expanded = expand_path(relative_path);
        
        assert_eq!(expanded, relative_path);
    }

    #[test]
    fn given_custom_env_var_when_expand_path_then_expands_correctly() {
        // Set a custom environment variable for testing
        env::set_var("TEST_DB_PATH", "/tmp/test");
        
        let env_path = "$TEST_DB_PATH/database.db";
        let expanded = expand_path(env_path);
        
        assert_eq!(expanded, "/tmp/test/database.db");
        assert!(!expanded.contains("$TEST_DB_PATH"));
        
        // Cleanup
        env::remove_var("TEST_DB_PATH");
    }
}
