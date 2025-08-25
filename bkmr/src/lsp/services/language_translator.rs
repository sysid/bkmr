use tower_lsp::lsp_types::Url;
use tracing::{debug, instrument};
use regex::{Regex, RegexBuilder};
use std::sync::OnceLock;

use crate::lsp::domain::{LanguageInfo, LanguageRegistry, Snippet};
use crate::domain::error::{DomainResult, DomainError};

// Pre-compiled regex patterns for performance (modern replacement for lazy_static)
static LINE_COMMENT_START: OnceLock<Regex> = OnceLock::new();
static LINE_COMMENT_END: OnceLock<Regex> = OnceLock::new();
static RUST_INDENT: OnceLock<Regex> = OnceLock::new();
static RAW_BLOCK_PATTERN: OnceLock<Regex> = OnceLock::new();

fn get_line_comment_start() -> &'static Regex {
    LINE_COMMENT_START.get_or_init(|| {
        Regex::new(r"^(\s*)//\s*(.*)$").expect("compile line comment start regex")
    })
}

fn get_line_comment_end() -> &'static Regex {
    LINE_COMMENT_END.get_or_init(|| {
        Regex::new(r"^(.+?)(\s+)//\s*(.*)$").expect("compile line comment end regex")
    })
}

fn get_rust_indent() -> &'static Regex {
    RUST_INDENT.get_or_init(|| {
        Regex::new(r"^( {4})+").expect("compile rust indentation regex")
    })
}

fn get_raw_block_pattern() -> &'static Regex {
    RAW_BLOCK_PATTERN.get_or_init(|| {
        RegexBuilder::new(r"\{%\s*raw\s*%\}(.*?)\{%\s*endraw\s*%\}")
            .dot_matches_new_line(true)
            .build()
            .expect("compile raw block regex")
    })
}

/// Service for translating Rust syntax patterns to target languages
pub struct LanguageTranslator;

impl LanguageTranslator {
    /// Filter raw blocks from content before processing
    fn filter_raw_blocks(content: &str) -> String {
        let raw_regex = get_raw_block_pattern();
        raw_regex.replace_all(content, |caps: &regex::Captures| {
            // Return just the content inside the raw block, without the markers
            caps.get(1).map_or("", |m| m.as_str()).to_string()
        }).to_string()
    }

    /// Translate Rust syntax patterns in universal snippets to target language
    #[instrument(skip(snippet))]
    pub fn translate_snippet(snippet: &Snippet, language_id: &str, uri: &Url) -> DomainResult<String> {
        // First, filter out raw block markers for all snippets (universal and regular)
        let filtered_content = Self::filter_raw_blocks(snippet.get_content());
        
        let content = if snippet.is_universal() {
            debug!("Processing universal snippet: {}", snippet.title);
            debug!("Original content: {:?}", snippet.get_content());
            debug!("Filtered content: {:?}", filtered_content);

            Self::translate_rust_patterns(&filtered_content, language_id, uri)?
        } else {
            // Regular snippet - return filtered content as-is
            filtered_content
        };

        debug!("Final translated content: {:?}", content);
        Ok(content)
    }

    /// Translate Rust syntax patterns in content to target language
    #[instrument(skip(content))]
    pub fn translate_rust_patterns(content: &str, language_id: &str, uri: &Url) -> DomainResult<String> {
        let target_lang = LanguageRegistry::get_language_info(language_id);

        debug!("Translating Rust patterns for language: {}", language_id);
        debug!("Input content: {:?}", content);
        debug!("Content length: {} bytes", content.len());

        // Use line-by-line processing to preserve newlines
        let mut processed_content =
            Self::translate_rust_patterns_line_by_line(content, &target_lang)
                .map_err(|e| DomainError::Other(format!("Failed to process content line by line: {}", e)))?;

        // Replace Rust block comments (/* */) with target language block comments
        if let Some((target_start, target_end)) = &target_lang.block_comment {
            let block_comment_regex = RegexBuilder::new(r"/\*(.*?)\*/")
                .dot_matches_new_line(true)
                .build()
                .map_err(|e| DomainError::Other(format!("Failed to compile block comment regex: {}", e)))?;

            processed_content = block_comment_regex
                .replace_all(&processed_content, |caps: &regex::Captures| {
                    format!("{}{}{}", target_start, &caps[1], target_end)
                })
                .to_string();
        }

        // Add file name replacement for simple relative path
        if processed_content.contains("{{ filename }}") {
            let filename = uri.path().split('/').next_back().unwrap_or("untitled");
            processed_content = processed_content.replace("{{ filename }}", filename);
        }

        debug!("Rust pattern translation complete");
        debug!("Final content: {:?}", processed_content);
        debug!("Final content length: {} bytes", processed_content.len());

        Ok(processed_content)
    }

    /// Process content line by line to preserve newlines properly
    fn translate_rust_patterns_line_by_line(
        content: &str,
        target_lang: &LanguageInfo,
    ) -> DomainResult<String> {
        let lines: Vec<&str> = content.split('\n').collect();
        let mut processed_lines = Vec::new();

        for line in lines {
            let mut processed_line = line.to_string();

            // Process line comments (//)
            if let Some(target_comment) = &target_lang.line_comment {
                // Start of line comments
                if let Some(captures) = get_line_comment_start().captures(line) {
                    processed_line = format!("{}{} {}", &captures[1], target_comment, &captures[2]);
                }
                // End of line comments (after code)
                else if let Some(captures) = get_line_comment_end().captures(line) {
                    processed_line = format!(
                        "{}{}{} {}",
                        &captures[1], &captures[2], target_comment, &captures[3]
                    );
                }
            } else if let Some((block_start, block_end)) = &target_lang.block_comment {
                // For languages without line comments, use block comments
                if let Some(captures) = get_line_comment_start().captures(line) {
                    processed_line = format!(
                        "{}{} {} {}",
                        &captures[1], block_start, &captures[2], block_end
                    );
                } else if let Some(captures) = get_line_comment_end().captures(line) {
                    processed_line = format!(
                        "{}{}{} {} {}",
                        &captures[1], &captures[2], block_start, &captures[3], block_end
                    );
                }
            }

            // Process indentation
            if target_lang.indent_char != "    " {
                if let Some(captures) = get_rust_indent().captures(&processed_line) {
                    let rust_indent_count = captures[0].len() / 4;
                    let new_indent = target_lang.indent_char.repeat(rust_indent_count);
                    processed_line = processed_line.replacen(&captures[0], &new_indent, 1);
                }
            }

            processed_lines.push(processed_line);
        }

        Ok(processed_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_universal_snippet_when_translating_then_processes_content() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test Universal Snippet".to_string(),
            "// This is a test".to_string(),
            "Test description".to_string(),
            vec!["universal".to_string(), "_snip_".to_string()],
        );
        let uri = Url::parse("file:///test.py").expect("parse URI");

        // Act
        let result = LanguageTranslator::translate_snippet(&snippet, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let translated = result.expect("valid translation result");
        assert!(translated.contains("# This is a test"));
    }

    #[test]
    fn given_regular_snippet_when_translating_then_returns_as_is() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test Regular Snippet".to_string(),
            "// This is a test".to_string(),
            "Test description".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );
        let uri = Url::parse("file:///test.py").expect("parse URI");

        // Act
        let result = LanguageTranslator::translate_snippet(&snippet, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let translated = result.expect("valid translation result");
        assert_eq!(translated, "// This is a test"); // No translation for non-universal
    }

    #[test]
    fn given_rust_line_comments_when_translating_to_python_then_converts_correctly() {
        // Arrange
        let uri = Url::parse("file:///test.py").expect("parse URI");
        let rust_content = r#"// This is a line comment
    // Indented comment
let x = 5; // End of line comment"#;

        // Act
        let result = LanguageTranslator::translate_rust_patterns(rust_content, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let python_result = result.expect("Python translation result");
        assert!(python_result.contains("# This is a line comment"));
        assert!(python_result.contains("    # Indented comment"));
        assert!(python_result.contains("let x = 5; # End of line comment"));
    }

    #[test]
    fn given_rust_block_comments_when_translating_to_python_then_converts_correctly() {
        // Arrange
        let uri = Url::parse("file:///test.py").expect("parse URI");
        let rust_content = r#"/* This is a block comment */
/*
Multi-line
block comment
*/"#;

        // Act
        let result = LanguageTranslator::translate_rust_patterns(rust_content, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let python_result = result.expect("Python translation result");
        assert!(python_result.contains("\"\"\" This is a block comment \"\"\""));
        assert!(python_result.contains("\"\"\"\nMulti-line\nblock comment\n\"\"\""));
    }

    #[test]
    fn given_rust_indentation_when_translating_to_go_then_converts_to_tabs() {
        // Arrange
        let uri = Url::parse("file:///test.go").expect("parse URI");
        let rust_content = r#"fn example() {
    let x = 5;
        let y = 10;
            let z = 15;
}"#;

        // Act
        let result = LanguageTranslator::translate_rust_patterns(rust_content, "go", &uri);

        // Assert
        assert!(result.is_ok());
        let go_result = result.expect("Go translation result");
        assert!(go_result.contains("fn example() {"));
        assert!(go_result.contains("\tlet x = 5;"));
        assert!(go_result.contains("\t\tlet y = 10;"));
        assert!(go_result.contains("\t\t\tlet z = 15;"));
    }

    #[test]
    fn given_filename_template_when_translating_then_replaces_correctly() {
        // Arrange
        let uri = Url::parse("file:///path/to/example.rs").expect("parse URI");
        let content = "// File: {{ filename }}";

        // Act
        let result = LanguageTranslator::translate_rust_patterns(content, "rust", &uri);

        // Assert
        assert!(result.is_ok());
        let translated = result.expect("valid translation result");
        assert!(translated.contains("// File: example.rs"));
    }

    #[test]
    fn given_content_with_raw_blocks_when_filtering_then_removes_markers() {
        // Arrange
        let content = "Before {% raw %}DATABASE_URL=${DATABASE_URL}{% endraw %} After";
        
        // Act
        let result = LanguageTranslator::filter_raw_blocks(content);
        
        // Assert
        assert_eq!(result, "Before DATABASE_URL=${DATABASE_URL} After");
    }

    #[test]
    fn given_multiline_raw_block_when_filtering_then_preserves_content() {
        // Arrange
        let content = r#"Setup:
{% raw %}
export DATABASE_URL=${DATABASE_URL}
export API_KEY=${API_KEY}
{% endraw %}
Done."#;
        
        // Act
        let result = LanguageTranslator::filter_raw_blocks(content);
        
        // Assert
        let expected = r#"Setup:

export DATABASE_URL=${DATABASE_URL}
export API_KEY=${API_KEY}

Done."#;
        assert_eq!(result, expected);
    }

    #[test]
    fn given_regular_snippet_with_raw_blocks_when_translating_then_filters_markers() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test Regular Snippet".to_string(),
            "Before {% raw %}${VAR}{% endraw %} After".to_string(),
            "Test description".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );
        let uri = Url::parse("file:///test.rs").expect("parse URI");

        // Act
        let result = LanguageTranslator::translate_snippet(&snippet, "rust", &uri);

        // Assert
        assert!(result.is_ok());
        let translated = result.expect("valid translation result");
        assert_eq!(translated, "Before ${VAR} After"); // Raw markers removed
    }

    #[test]
    fn given_universal_snippet_with_raw_blocks_when_translating_then_filters_and_translates() {
        // Arrange
        let snippet = Snippet::new(
            1,
            "Test Universal Snippet".to_string(),
            "// Comment {% raw %}${DATABASE_URL}{% endraw %}".to_string(),
            "Test description".to_string(),
            vec!["universal".to_string(), "_snip_".to_string()],
        );
        let uri = Url::parse("file:///test.py").expect("parse URI");

        // Act
        let result = LanguageTranslator::translate_snippet(&snippet, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let translated = result.expect("valid translation result");
        assert_eq!(translated, "# Comment ${DATABASE_URL}"); // Markers removed and comment translated
    }

    #[test]
    fn given_content_without_raw_blocks_when_filtering_then_unchanged() {
        // Arrange
        let content = "Normal content without raw blocks";
        
        // Act
        let result = LanguageTranslator::filter_raw_blocks(content);
        
        // Assert
        assert_eq!(result, content);
    }

    #[test]
    fn given_multiple_raw_blocks_when_filtering_then_removes_all_markers() {
        // Arrange
        let content = "First {% raw %}${VAR1}{% endraw %} middle {% raw %}${VAR2}{% endraw %} end";
        
        // Act
        let result = LanguageTranslator::filter_raw_blocks(content);
        
        // Assert
        assert_eq!(result, "First ${VAR1} middle ${VAR2} end");
    }
}