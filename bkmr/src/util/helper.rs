// src/util/helper.rs
use regex::Regex;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::env;
use tracing::debug;
use md5;

/// Ensure a vector of strings contains only integers
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
    use std::path::Path;

    // Compile a regex to find a suffix pattern like ":<integer>"
    let regex = Regex::new(":\\d+$").unwrap();

    // Remove the suffix if present
    let p_without_suffix = regex.replace(p, "");

    let expanded_path = shellexpand::full(&p_without_suffix).ok()?;
    let path = Path::new(expanded_path.as_ref());

    let abs_path = path.canonicalize().ok()?;
    abs_path.to_str().map(|s| s.to_string())
}

/// Calculate MD5 hash of content
pub fn calc_content_hash(content: &str) -> Vec<u8> {
    md5::compute(content).0.to_vec()
}

/// Interactive confirmation prompt
pub fn confirm(prompt: &str) -> bool {
    print!("{} (y/N): ", prompt);
    io::stdout().flush().unwrap(); // Ensure the prompt is displayed immediately

    let mut user_input = String::new();
    io::stdin()
        .read_line(&mut user_input)
        .expect("Failed to read line");

    matches!(user_input.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Check if a website is accessible
pub fn check_website(url: &str, timeout_milliseconds: u64) -> (bool, u128) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_milliseconds))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new()); // Fallback to default client in case of builder failure

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

/// Check if environment variable is set
pub fn is_env_var_set(env_var_name: &str) -> bool {
    env::var(env_var_name).is_ok()
}

/// Extract filename from: $HOME/bla/file.md:0
pub fn extract_filename(input: &str) -> String {
    use std::path::Path;

    // Attempt to split the input string by ':' to handle potential line indicators
    let parts: Vec<&str> = input.split(':').collect();
    let path_str = parts[0]; // The path part of the input

    // Use the Path type to manipulate file paths
    let path = Path::new(path_str);

    // Extract the filename, if it exists, and convert it to a String
    path.file_name()
        .map_or(input.to_string(), |filename| filename.to_string_lossy().to_string())
}
