use crate::domain::tag::Tag;
use crate::lsp::domain::LanguageRegistry;
use std::collections::HashSet;
use tower_lsp_server::ls_types::{Position, Range, Uri};

/// Represents a completion query extracted from the document
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionQuery {
    pub text: String,
    pub range: Range,
}

impl CompletionQuery {
    pub fn new(text: String, range: Range) -> Self {
        Self { text, range }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Context for completion requests
#[derive(Debug, Clone)]
pub struct CompletionContext {
    pub uri: Uri,
    pub position: Position,
    pub language_id: Option<String>,
    pub query: Option<CompletionQuery>,
}

impl CompletionContext {
    pub fn new(uri: Uri, position: Position, language_id: Option<String>) -> Self {
        Self {
            uri,
            position,
            language_id,
            query: None,
        }
    }

    pub fn with_query(mut self, query: CompletionQuery) -> Self {
        self.query = Some(query);
        self
    }

    pub fn has_query(&self) -> bool {
        self.query.is_some()
    }

    pub fn get_query_text(&self) -> Option<&str> {
        self.query.as_ref().map(|q| q.text.as_str())
    }

    pub fn get_replacement_range(&self) -> Option<Range> {
        self.query.as_ref().map(|q| q.range)
    }
}

/// Configuration for snippet filtering
#[derive(Debug, Clone)]
pub struct SnippetFilter {
    pub language_id: Option<String>,
    pub query_prefix: Option<String>,
    pub max_results: usize,
    pub enable_interpolation: bool,
}

impl SnippetFilter {
    pub fn new(
        language_id: Option<String>,
        query_prefix: Option<String>,
        max_results: usize,
        enable_interpolation: bool,
    ) -> Self {
        Self {
            language_id,
            query_prefix,
            max_results,
            enable_interpolation,
        }
    }

    /// Tags of which a snippet must carry at least one to be offered: the language's
    /// aliases plus `universal`. None when the language is unknown (offer all snippets).
    ///
    /// Matched exactly by the repository, not via FTS: FTS tokenizes tags
    /// (`rust-analyzer` -> `rust`, `analyzer`) and rejects hyphenated barewords.
    pub fn language_tags(&self) -> Option<HashSet<Tag>> {
        let lang = self
            .language_id
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())?;
        Some(
            LanguageRegistry::tags_for_language(lang)
                .iter()
                .map(String::as_str)
                .chain(std::iter::once("universal"))
                .filter_map(|t| Tag::new(t).ok())
                .collect(),
        )
    }
}

impl Default for SnippetFilter {
    fn default() -> Self {
        Self {
            language_id: None,
            query_prefix: None,
            max_results: 50,
            enable_interpolation: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Position;

    #[test]
    fn given_text_and_range_when_creating_completion_query_then_stores_correctly() {
        // Arrange
        let text = "hello".to_string();
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 5,
            },
        };

        // Act
        let query = CompletionQuery::new(text.clone(), range);

        // Assert
        assert_eq!(query.text, "hello");
        assert_eq!(query.range, range);
    }

    #[test]
    fn given_empty_text_when_checking_is_empty_then_returns_true() {
        // Arrange
        let query = CompletionQuery::new(
            String::new(),
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
        );

        // Act
        let is_empty = query.is_empty();

        // Assert
        assert!(is_empty);
    }

    #[test]
    fn given_completion_context_when_adding_query_then_updates_correctly() {
        // Arrange
        let uri = "file:///test.rs".parse::<Uri>().expect("parse URL");
        let position = Position {
            line: 0,
            character: 5,
        };
        let language_id = Some("rust".to_string());
        let query = CompletionQuery::new(
            "test".to_string(),
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 4,
                },
            },
        );

        // Act
        let context = CompletionContext::new(uri.clone(), position, language_id.clone())
            .with_query(query.clone());

        // Assert
        assert_eq!(context.uri, uri);
        assert_eq!(context.position, position);
        assert_eq!(context.language_id, language_id);
        assert!(context.has_query());
        assert_eq!(context.get_query_text(), Some("test"));
    }

    #[test]
    fn given_language_id_when_getting_language_tags_then_includes_aliases_and_universal() {
        // Arrange
        let filter = SnippetFilter::new(Some("rust".to_string()), None, 50, true);

        // Act
        let tags = filter.language_tags().expect("language tags");

        // Assert
        let values: HashSet<&str> = tags.iter().map(|t| t.value()).collect();
        assert_eq!(values, HashSet::from(["rust", "rs", "universal"]));
    }

    #[test]
    fn given_no_language_id_when_getting_language_tags_then_returns_none() {
        // Arrange
        let filter = SnippetFilter::new(None, None, 50, true);

        // Act
        let tags = filter.language_tags();

        // Assert
        assert!(tags.is_none());
    }

    #[test]
    fn given_empty_language_id_when_getting_language_tags_then_returns_none() {
        // Arrange
        let filter = SnippetFilter::new(Some("  ".to_string()), None, 50, true);

        // Act
        let tags = filter.language_tags();

        // Assert
        assert!(tags.is_none());
    }
}
