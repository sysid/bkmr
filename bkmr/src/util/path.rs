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
        &["tests/resources/bkmr.v1.db", "tests/resources/bkmr.v2.db"],
        tempdir.path(),
        &options,
    )
    .expect("Failed to copy test project directory");

    tempdir.into_path()
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
    fn test_abspath_removes_suffix() {
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
    fn test_extract_filename() {
        let input = "/home/user/docs/report.pdf:0";
        let filename = extract_filename(input);
        assert_eq!(filename, "report.pdf");
    }
}
