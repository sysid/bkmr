#[cfg(feature = "lsp")]
use tower_lsp::lsp_types::{Position, Range, Url};

/// Represents a completion query extracted from the document
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionQuery {
    pub text: String,
    #[cfg(feature = "lsp")]
    pub range: Range,
}

impl CompletionQuery {
    #[cfg(feature = "lsp")]
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
    #[cfg(feature = "lsp")]
    pub uri: Url,
    #[cfg(feature = "lsp")]
    pub position: Position,
    pub language_id: Option<String>,
    pub query: Option<CompletionQuery>,
}

impl CompletionContext {
    #[cfg(feature = "lsp")]
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

    #[cfg(feature = "lsp")]
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
}

impl SnippetFilter {
    pub fn new(
        language_id: Option<String>,
        query_prefix: Option<String>,
        max_results: usize,
    ) -> Self {
        Self {
            language_id,
            query_prefix,
            max_results,
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
        }
    }
}