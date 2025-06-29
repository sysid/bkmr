// src/util/helper.rs
use chrono::{TimeZone, Utc};
use md5;
use std::io::{self, IsTerminal, Write};

/// Ensure a vector of strings contains only integers
pub fn ensure_int_vector(vec: &[String]) -> Option<Vec<i32>> {
    vec.iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .map(|mut v| {
            v.sort();
            v
        })
        .ok()
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

pub fn is_stdout_piped() -> bool {
    !io::stdout().is_terminal()
}

pub fn is_stderr_piped() -> bool {
    !io::stderr().is_terminal()
}

/// Format file path for display, truncating if necessary
pub fn format_file_path(path: &str, max_length: usize) -> String {
    if path.len() <= max_length {
        path.to_string()
    } else {
        // Keep base path variables intact if present
        if path.starts_with('$') {
            if let Some(slash_pos) = path.find('/') {
                let (var_part, path_part) = path.split_at(slash_pos);
                if var_part.len() + 3 < max_length {
                    // Try to keep the variable and truncate the path
                    let remaining_length = max_length - var_part.len() - 3; // 3 for "..."
                    if path_part.len() > remaining_length {
                        // Take last part of the path that fits
                        let start = path_part.len() - remaining_length;
                        format!("{}...{}", var_part, &path_part[start..])
                    } else {
                        path.to_string()
                    }
                } else {
                    // Even the variable is too long, just truncate from the end
                    format!("...{}", &path[path.len() - (max_length - 3)..])
                }
            } else {
                // Just a variable, no path
                path.to_string()
            }
        } else {
            // Regular path, truncate from the beginning
            format!("...{}", &path[path.len() - (max_length - 3)..])
        }
    }
}

/// Format modification time as absolute timestamp
pub fn format_mtime(mtime: i32) -> String {
    let datetime = Utc.timestamp_opt(mtime as i64, 0);
    match datetime {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "Invalid timestamp".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_int_vector_valid() {
        let input = vec!["3".to_string(), "1".to_string(), "2".to_string()];
        let result = ensure_int_vector(&input);
        // Expected vector is sorted in ascending order.
        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_ensure_int_vector_invalid() {
        let input = vec!["3".to_string(), "abc".to_string(), "2".to_string()];
        let result = ensure_int_vector(&input);
        assert!(result.is_none());
    }

    #[test]
    fn test_calc_content_hash() {
        let content = "hello world";
        let hash = calc_content_hash(content);
        // Using md5 directly to get the expected hash.
        let expected = md5::compute(content);
        assert_eq!(hash, expected.0.to_vec());
    }

    #[test]
    fn test_format_file_path() {
        // Short paths should not be truncated
        assert_eq!(format_file_path("/home/user/file.txt", 120), "/home/user/file.txt");
        
        // Long paths should be truncated from the beginning
        let long_path = "/home/user/very/long/path/to/some/deeply/nested/directory/structure/with/file.txt";
        let formatted = format_file_path(long_path, 30);
        assert!(formatted.starts_with("..."));
        assert!(formatted.ends_with("file.txt"));
        assert_eq!(formatted.len(), 30);
        
        // Base path variables should be preserved
        assert_eq!(format_file_path("$HOME/scripts/test.sh", 120), "$HOME/scripts/test.sh");
        
        // Long paths with base path variables
        let var_path = "$SCRIPTS_HOME/very/long/path/to/some/script.sh";
        let formatted = format_file_path(var_path, 30);
        assert!(formatted.starts_with("$SCRIPTS_HOME..."));
        assert!(formatted.ends_with("script.sh"));
    }

    #[test]
    fn test_format_mtime() {
        // Test with a known timestamp
        let timestamp = 1704067200; // 2024-01-01 00:00:00 UTC
        assert_eq!(format_mtime(timestamp), "2024-01-01 00:00:00");
        
        // Test with invalid timestamp (negative)
        assert_eq!(format_mtime(-1), "1969-12-31 23:59:59");
    }
}
