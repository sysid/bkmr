// src/domain/search.rs
use crate::domain::bookmark::Bookmark;

/// Represents a semantic search query and parameters.
/// The actual search logic lives in BookmarkServiceImpl which uses VectorRepository.
#[derive(Debug, Clone)]
pub struct SemanticSearch {
    /// The text query to search for
    pub query: String,

    /// Maximum number of results to return
    pub limit: Option<usize>,
}

/// Result of a semantic search, including the bookmark and its similarity score
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    /// The bookmark that matched the search
    pub bookmark: Bookmark,

    /// Similarity score (0.0 to 1.0, higher is better)
    pub similarity: f32,
}

impl SemanticSearch {
    /// Create a new semantic search query
    pub fn new(query: impl Into<String>, limit: Option<usize>) -> Self {
        Self {
            query: query.into(),
            limit,
        }
    }
}

impl SemanticSearchResult {
    /// Format the similarity score as a percentage
    pub fn similarity_percentage(&self) -> String {
        format!("{:.1}%", self.similarity * 100.0)
    }

    /// Create a new semantic search result with additional display metadata
    pub fn new(bookmark: Bookmark, similarity: f32) -> Self {
        Self {
            bookmark,
            similarity,
        }
    }

    /// Simple display text for semantic search results in fzf interface
    /// This provides basic display formatting - enhanced formatting should be implemented
    /// at the application layer where services are available
    pub fn display(&self) -> String {
        use crossterm::style::Stylize;

        let id = self.bookmark.id.unwrap_or(0);
        let title = &self.bookmark.title;
        let url = &self.bookmark.url;
        let binding = self.bookmark.formatted_tags();
        let tags_str = binding.trim_matches(',');
        let similarity = format!("{:.1}%", self.similarity * 100.0);

        // Format with colors similar to main branch implementation
        let tags_display = if !tags_str.is_empty() {
            format!(" [{}]", tags_str.magenta())
        } else {
            String::new()
        };

        let action_display = " (default)".cyan();

        format!(
            "{}: {} <{}> ({}%){}{}",
            id.to_string().blue(),
            title.clone().green(),
            url.clone().yellow(),
            similarity.cyan(),
            action_display,
            tags_display
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::util::testing::init_test_env;
    use std::collections::HashSet;

    fn create_test_bookmark(title: &str, content: &str, has_embedding: bool) -> Bookmark {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let embedder = &crate::infrastructure::embeddings::DummyEmbedding;

        let mut bookmark =
            Bookmark::new("https://example.com", title, content, tags, embedder).unwrap();

        bookmark.set_embeddable(has_embedding);
        bookmark
    }

    #[test]
    fn given_semantic_search_when_new_then_stores_query_and_limit() {
        let search = SemanticSearch::new("test query", Some(5));
        assert_eq!(search.query, "test query");
        assert_eq!(search.limit, Some(5));
    }

    #[test]
    fn given_semantic_search_when_no_limit_then_limit_is_none() {
        let search = SemanticSearch::new("test query", None);
        assert_eq!(search.query, "test query");
        assert_eq!(search.limit, None);
    }

    #[test]
    fn given_similarity_score_when_format_percentage_then_returns_correct_format() {
        let _ = init_test_env();
        let bookmark = create_test_bookmark("Test", "Content", true);

        let result = SemanticSearchResult {
            bookmark,
            similarity: 0.756,
        };

        assert_eq!(result.similarity_percentage(), "75.6%");
    }
}
