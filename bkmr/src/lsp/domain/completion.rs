use tower_lsp::lsp_types::{Position, Range, Url};

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
    pub uri: Url,
    pub position: Position,
    pub language_id: Option<String>,
    pub query: Option<CompletionQuery>,
}

impl CompletionContext {
    pub fn new(uri: Url, position: Position, language_id: Option<String>) -> Self {
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

    /// Build FTS query for snippets that includes both language-specific and universal snippets
    pub fn build_fts_query(&self) -> Option<String> {
        if let Some(ref lang) = self.language_id {
            if !lang.trim().is_empty() {
                // Query for either (language AND _snip_) OR (universal AND _snip_)
                return Some(format!(
                    r#"(tags:{} AND tags:"_snip_") OR (tags:universal AND tags:"_snip_")"#,
                    lang
                ));
            }
        }
        // Fallback: just get all snippets with _snip_ tag
        Some(r#"tags:"_snip_""#.to_string())
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
    use tower_lsp::lsp_types::Position;

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
        let uri = Url::parse("file:///test.rs").expect("parse URL");
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
    fn given_language_id_when_building_fts_query_then_includes_universal_snippets() {
        // Arrange
        let filter = SnippetFilter::new(Some("rust".to_string()), None, 50, true);

        // Act
        let query = filter.build_fts_query();

        // Assert
        assert_eq!(
            query,
            Some(
                r#"(tags:rust AND tags:"_snip_") OR (tags:universal AND tags:"_snip_")"#
                    .to_string()
            )
        );
    }

    #[test]
    fn given_no_language_id_when_building_fts_query_then_returns_basic_query() {
        // Arrange
        let filter = SnippetFilter::new(None, None, 50, true);

        // Act
        let query = filter.build_fts_query();

        // Assert
        assert_eq!(query, Some(r#"tags:"_snip_""#.to_string()));
    }

    #[test]
    fn given_empty_language_id_when_building_fts_query_then_returns_basic_query() {
        // Arrange
        let filter = SnippetFilter::new(Some("".to_string()), None, 50, true);

        // Act
        let query = filter.build_fts_query();

        // Assert
        assert_eq!(query, Some(r#"tags:"_snip_""#.to_string()));
    }
}
