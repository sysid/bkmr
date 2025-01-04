use std::io::Write;
use std::time::{Duration, Instant};
use std::{env, io};
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use camino_tempfile::tempdir;
use fs_extra::{copy_items, dir};
use regex::Regex;
use reqwest::blocking;
use tracing::debug;


/// Prepare test directory with test data and return path
pub fn temp_dir() -> Utf8PathBuf {
    let tempdir = tempdir().unwrap();
    let options = dir::CopyOptions::new(); //Initialize default values for CopyOptions
    copy_items(
        &["tests/resources/bkmr.v1.db", "tests/resources/bkmr.v2.db"],
        &tempdir,
        &options,
    )
    .expect("Failed to copy test project directory");

    tempdir.into_path()
}

#[allow(clippy::ptr_arg)]
pub fn ensure_int_vector(vec: &Vec<String>) -> Option<Vec<i32>> {
    vec.iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .map(|mut v| {
            v.sort();
            v
        })
        .ok()
}

/// resolves existing path and follows symlinks, returns None if path does not exist
/// also removes suffix like ":1" or ":0" from the path if present
pub fn abspath(p: &str) -> Option<String> {
    // Compile a regex to find a suffix pattern like ":<integer>"
    let regex = Regex::new(":\\d+$").unwrap();

    // Remove the suffix if present
    let p_without_suffix = regex.replace(p, "");

    let abs_p = shellexpand::full(&p_without_suffix)
        .ok()
        .and_then(|x| Utf8Path::new(x.as_ref()).canonicalize_utf8().ok())
        .map(|p| p.into_string());

    debug!("{:?} -> {:?}", p, abs_p);
    abs_p
}

pub fn calc_content_hash(content: &str) -> Vec<u8> {
    md5::compute(content).0.to_vec()
}

pub fn confirm(prompt: &str) -> bool {
    print!("{} (y/N): ", prompt);
    io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately

    let mut user_input = String::new();
    io::stdin()
        .read_line(&mut user_input)
        .expect("Failed to read line");

    matches!(user_input.trim().to_lowercase().as_str(), "y" | "yes")
}

pub fn check_website(url: &str, timeout_milliseconds: u64) -> (bool, u128) {
    let client = blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_milliseconds))
        .build()
        .unwrap_or_else(|_| blocking::Client::new()); // Fallback to default client in case of builder failure

    let start = Instant::now();
    let response = client.head(url).send();

    match response {
        Ok(resp) if resp.status().is_success() => {
            let duration = start.elapsed().as_millis();
            (true, duration)
        }
        _ => (false, 0), // Return false and 0 duration in case of error or non-success status
    }
}

pub fn is_env_var_set(env_var_name: &str) -> bool {
    env::var(env_var_name).is_ok()
}

/// Extract filename from: $HOME/bla/file.md:0
pub fn extract_filename(input: &str) -> String {
    // Attempt to split the input string by ':' to handle potential line indicators
    let parts: Vec<&str> = input.split(':').collect();
    let path_str = parts[0]; // The path part of the input

    // Use the Path type to manipulate file paths
    let path = Utf8Path::new(path_str);

    // Extract the filename, if it exists, and convert it to a String
    path.file_name()
        .map_or(input.to_string(), |filename| filename.to_string())
}

#[cfg(test)]
mod test {
    use rstest::*;

    // use log::debug;
    use super::*;

    #[rstest]
    fn test_extract_filename() {
        // Examples
        let example1 = "$HOME/bla/file.md:0";
        let example2 = "$HOME/bla/file.md";
        let example3 = "just_a_string";

        assert_eq!(extract_filename(example1), "file.md");
        assert_eq!(extract_filename(example2), "file.md");
        assert_eq!(extract_filename(example3), "just_a_string");
    }

    #[rstest]
    #[case(vec ! ["1".to_string(), "2".to_string(), "3".to_string()], Some(vec ! [1, 2, 3]))]
    #[case(vec ! ["3".to_string(), "1".to_string(), "2".to_string()], Some(vec ! [1, 2, 3]))]
    #[case(vec ! ["a".to_string(), "2".to_string(), "3".to_string()], None)]
    #[case(vec ! [], Some(vec ! []))]
    fn test_ensure_int_vector(#[case] x: Vec<String>, #[case] expected: Option<Vec<i32>>) {
        assert_eq!(ensure_int_vector(&x), expected);
    }

    // Tests are fragile, because they depend on machine specific setup
    #[rstest]
    #[case("", None)]
    #[ignore = "fragile"]
    #[case("~/dev/binx", Some("/Users/Q187392/dev/s/private/devops-binx".to_string()))] // link resolved
    #[ignore = "fragile"]
    #[case("$HOME/dev/binx", Some("/Users/Q187392/dev/s/private/devops-binx".to_string()))]
    #[case("https://www.google.com", None)]
    #[ignore = "fragile"]
    #[case("./tests/resources/bkmr.pptx", Some("/Users/Q187392/dev/s/public/bkmr/bkmr/tests/resources/bkmr.pptx".to_string()))] // link resolved
    fn test_abspath(#[case] x: &str, #[case] expected: Option<String>) {
        assert_eq!(abspath(x), expected);
    }

    #[rstest]
    fn abspath_returns_none_if_path_does_not_exist() {
        let input = "/non/existent/path";
        assert_eq!(abspath(input), None);
    }

    #[rstest]
    fn abspath_removes_suffix_from_path() {
        let input = "/tmp/file:1";
        let expected = Some("/private/tmp/file".to_string()); // macos gotcha: /private

        // Create the file
        std::fs::File::create("/tmp/file").unwrap();

        assert_eq!(abspath(input), expected);

        // Delete the file
        std::fs::remove_file("/tmp/file").unwrap();
    }

    #[rstest]
    fn abspath_returns_none_if_input_is_empty() {
        let input = "";
        assert_eq!(abspath(input), None);
    }

    #[rstest]
    #[ignore = "external dependency, run manual"]
    fn test_check_website() {
        let (success, duration) = check_website("https://www.google.com", 1000);
        println!("Success: {}, Duration: {}", success, duration);
        assert!(success);
        assert!(duration > 0);
    }

    #[rstest]
    // #[ignore = "external dependency, run manual"]
    fn test_is_env_var_set() {
        assert!(is_env_var_set("HOME"));
    }
}
