/// Language-specific information for code pattern translation
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageInfo {
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub indent_char: String,
}

impl LanguageInfo {
    pub fn new(
        line_comment: Option<String>,
        block_comment: Option<(String, String)>,
        indent_char: String,
    ) -> Self {
        Self {
            line_comment,
            block_comment,
            indent_char,
        }
    }
}

/// Language registry for mapping language IDs to language information
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Get language information for a specific language ID
    pub fn get_language_info(language_id: &str) -> LanguageInfo {
        match language_id.to_lowercase().as_str() {
            "rust" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "javascript" | "js" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "  ".to_string(),
            ),
            "typescript" | "ts" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "  ".to_string(),
            ),
            "python" => LanguageInfo::new(
                Some("#".to_string()),
                Some(("\"\"\"".to_string(), "\"\"\"".to_string())),
                "    ".to_string(),
            ),
            "go" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "\t".to_string(),
            ),
            "java" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "c" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "cpp" | "c++" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "html" => LanguageInfo::new(
                None,
                Some(("<!--".to_string(), "-->".to_string())),
                "  ".to_string(),
            ),
            "css" => LanguageInfo::new(
                None,
                Some(("/*".to_string(), "*/".to_string())),
                "  ".to_string(),
            ),
            "scss" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "  ".to_string(),
            ),
            "ruby" => LanguageInfo::new(
                Some("#".to_string()),
                Some(("=begin".to_string(), "=end".to_string())),
                "  ".to_string(),
            ),
            "php" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "swift" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "kotlin" => LanguageInfo::new(
                Some("//".to_string()),
                Some(("/*".to_string(), "*/".to_string())),
                "    ".to_string(),
            ),
            "shell" | "bash" | "sh" => {
                LanguageInfo::new(Some("#".to_string()), None, "    ".to_string())
            }
            "yaml" | "yml" => LanguageInfo::new(Some("#".to_string()), None, "  ".to_string()),
            "json" => LanguageInfo::new(None, None, "  ".to_string()),
            "markdown" | "md" => LanguageInfo::new(
                None,
                Some(("<!--".to_string(), "-->".to_string())),
                "  ".to_string(),
            ),
            "xml" => LanguageInfo::new(
                None,
                Some(("<!--".to_string(), "-->".to_string())),
                "  ".to_string(),
            ),
            "vim" | "viml" => LanguageInfo::new(Some("\"".to_string()), None, "  ".to_string()),
            // Default fallback for unknown languages
            _ => LanguageInfo::new(Some("#".to_string()), None, "    ".to_string()),
        }
    }

    /// Get legacy comment syntax for backward compatibility
    pub fn get_comment_syntax(file_path: &str) -> &'static str {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let language_id = Self::extension_to_language_id(extension);

        match language_id {
            "rust" | "javascript" | "typescript" | "go" | "java" | "c" | "cpp" | "swift"
            | "kotlin" | "scss" | "php" => "//",
            "python" | "shell" | "yaml" => "#",
            "html" | "markdown" | "xml" => "<!--",
            "css" => "/*",
            "vim" => "\"",
            _ => "#",
        }
    }

    /// Map file extension to language ID
    fn extension_to_language_id(extension: &str) -> &str {
        match extension {
            "rs" => "rust",
            "js" | "mjs" => "javascript",
            "ts" | "tsx" => "typescript",
            "py" | "pyw" => "python",
            "go" => "go",
            "java" => "java",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" => "cpp",
            "html" | "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "sh" | "bash" | "zsh" => "shell",
            "yaml" | "yml" => "yaml",
            "json" => "json",
            "md" | "markdown" => "markdown",
            "xml" => "xml",
            "vim" => "vim",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_rust_language_when_getting_info_then_returns_correct_settings() {
        // Arrange
        let language_id = "rust";

        // Act
        let language_info = LanguageRegistry::get_language_info(language_id);

        // Assert
        assert_eq!(language_info.line_comment, Some("//".to_string()));
        assert_eq!(
            language_info.block_comment,
            Some(("/*".to_string(), "*/".to_string()))
        );
        assert_eq!(language_info.indent_char, "    ");
    }

    #[test]
    fn given_python_language_when_getting_info_then_returns_correct_settings() {
        // Arrange
        let language_id = "python";

        // Act
        let language_info = LanguageRegistry::get_language_info(language_id);

        // Assert
        assert_eq!(language_info.line_comment, Some("#".to_string()));
        assert_eq!(
            language_info.block_comment,
            Some(("\"\"\"".to_string(), "\"\"\"".to_string()))
        );
        assert_eq!(language_info.indent_char, "    ");
    }

    #[test]
    fn given_unknown_language_when_getting_info_then_returns_default_settings() {
        // Arrange
        let language_id = "unknownlang";

        // Act
        let language_info = LanguageRegistry::get_language_info(language_id);

        // Assert
        assert_eq!(language_info.line_comment, Some("#".to_string()));
        assert_eq!(language_info.block_comment, None);
        assert_eq!(language_info.indent_char, "    ");
    }

    #[test]
    fn given_rust_file_when_getting_comment_syntax_then_returns_double_slash() {
        // Arrange
        let file_path = "test.rs";

        // Act
        let comment_syntax = LanguageRegistry::get_comment_syntax(file_path);

        // Assert
        assert_eq!(comment_syntax, "//");
    }

    #[test]
    fn given_python_file_when_getting_comment_syntax_then_returns_hash() {
        // Arrange
        let file_path = "test.py";

        // Act
        let comment_syntax = LanguageRegistry::get_comment_syntax(file_path);

        // Assert
        assert_eq!(comment_syntax, "#");
    }
}
