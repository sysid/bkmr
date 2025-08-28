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

/// Create a valid shell function name from a bookmark title
pub fn create_shell_function_name(title: &str) -> String {
    let cleaned_name = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c.to_ascii_lowercase() // Preserve hyphens
            } else if c.is_whitespace() || c == '_' {
                '_' // Only spaces and underscores become underscores
            } else {
                // Skip other invalid characters
                '\0'
            }
        })
        .filter(|&c| c != '\0')
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    // Ensure we have a valid function name
    if cleaned_name.is_empty() {
        "shell_script".to_string()
    } else if cleaned_name.chars().next().unwrap().is_ascii_digit() {
        // Shell function names can't start with a digit
        format!("script-{}", cleaned_name)
    } else {
        cleaned_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_valid_string_numbers_when_ensure_int_vector_then_returns_sorted_integers() {
        let input = vec!["3".to_string(), "1".to_string(), "2".to_string()];
        let result = ensure_int_vector(&input);
        // Expected vector is sorted in ascending order.
        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn given_invalid_string_numbers_when_ensure_int_vector_then_returns_none() {
        let input = vec!["3".to_string(), "abc".to_string(), "2".to_string()];
        let result = ensure_int_vector(&input);
        assert!(result.is_none());
    }

    #[test]
    fn given_content_string_when_calc_content_hash_then_returns_sha256_hash() {
        let content = "hello world";
        let hash = calc_content_hash(content);
        // Using md5 directly to get the expected hash.
        let expected = md5::compute(content);
        assert_eq!(hash, expected.0.to_vec());
    }

    #[test]
    fn given_file_path_when_format_file_path_then_truncates_long_paths() {
        // Short paths should not be truncated
        assert_eq!(
            format_file_path("/home/user/file.txt", 120),
            "/home/user/file.txt"
        );

        // Long paths should be truncated from the beginning
        let long_path =
            "/home/user/very/long/path/to/some/deeply/nested/directory/structure/with/file.txt";
        let formatted = format_file_path(long_path, 30);
        assert!(formatted.starts_with("..."));
        assert!(formatted.ends_with("file.txt"));
        assert_eq!(formatted.len(), 30);

        // Base path variables should be preserved
        assert_eq!(
            format_file_path("$HOME/scripts/test.sh", 120),
            "$HOME/scripts/test.sh"
        );

        // Long paths with base path variables
        let var_path = "$SCRIPTS_HOME/very/long/path/to/some/script.sh";
        let formatted = format_file_path(var_path, 30);
        assert!(formatted.starts_with("$SCRIPTS_HOME..."));
        assert!(formatted.ends_with("script.sh"));
    }

    #[test]
    fn given_unix_timestamp_when_format_mtime_then_returns_formatted_datetime() {
        // Test with a known timestamp
        let timestamp = 1704067200; // 2024-01-01 00:00:00 UTC
        assert_eq!(format_mtime(timestamp), "2024-01-01 00:00:00");

        // Test with invalid timestamp (negative)
        assert_eq!(format_mtime(-1), "1969-12-31 23:59:59");
    }

    #[test]
    fn given_various_titles_when_create_shell_function_name_then_returns_valid_function_name() {
        // Test basic alphanumeric names
        assert_eq!(create_shell_function_name("backup_script"), "backup_script");
        assert_eq!(create_shell_function_name("backup-script"), "backup-script");

        // Test spaces become underscores
        assert_eq!(create_shell_function_name("Deploy Script"), "deploy_script");

        // Test edge case with digits at start
        assert_eq!(create_shell_function_name("2fa-setup"), "script-2fa-setup");

        // Test invalid characters are removed
        assert_eq!(create_shell_function_name("test@#$script!"), "testscript");

        // Test empty result fallback
        assert_eq!(create_shell_function_name("@#$%"), "shell_script");

        // Test trimming underscores
        assert_eq!(create_shell_function_name("__test__"), "test");
    }
}
