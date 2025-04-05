// src/util/helper.rs
use md5;
use std::io::{self, Write};

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
}
