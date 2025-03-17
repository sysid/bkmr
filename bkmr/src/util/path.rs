// src/util/path.rs
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;

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
    path.file_name()
        .map_or(input.to_string(), |filename| filename.to_string_lossy().to_string())
}
