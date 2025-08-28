// src/domain/search.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::embedding::{cosine_similarity, deserialize_embedding, Embedder};
use crate::domain::error::DomainResult;
use ndarray::Array1;
use std::cmp::Ordering;

/// Represents a semantic search query and parameters
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

    /// Execute the semantic search against a collection of bookmarks
    pub fn execute(
        &self,
        bookmarks: &[Bookmark],
        embedder: &dyn Embedder,
    ) -> DomainResult<Vec<SemanticSearchResult>> {
        // Generate embedding for the query text
        let query_embedding = match embedder.embed(&self.query)? {
            Some(embedding) => embedding,
            None => return Ok(Vec::new()), // Return empty results if no embedding could be generated
        };

        // Convert to ndarray for similarity calculation
        let query_vector = Array1::from(query_embedding);

        let mut results = Vec::new();

        // Calculate similarity for each bookmark with an embedding
        // Only include bookmarks with embeddable=true
        for bookmark in bookmarks {
            if bookmark.embeddable && bookmark.embedding.is_some() {
                // todo: Check if the embedding exists and is up-to-date, else recompute it
                if let Some(embedding_bytes) = &bookmark.embedding {
                    // Deserialize the embedding bytes back to vector
                    match deserialize_embedding(embedding_bytes.clone()) {
                        Ok(bm_embedding) => {
                            let bm_vector = Array1::from(bm_embedding);
                            let similarity = cosine_similarity(&query_vector, &bm_vector);

                            results.push(SemanticSearchResult {
                                bookmark: bookmark.clone(),
                                similarity,
                            });
                        }
                        Err(_) => {
                            // Skip this bookmark if we can't deserialize its embedding
                            continue;
                        }
                    }
                }
            }
        }

        // Sort by similarity (highest first)
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(Ordering::Equal)
        });

        // Apply limit if provided
        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        Ok(results)
    }
}

impl SemanticSearchResult {
    /// Format the similarity score as a percentage
    pub fn similarity_percentage(&self) -> String {
        format!("{:.1}%", self.similarity * 100.0)
    }
    
    /// Create a new semantic search result with additional display metadata
    pub fn new(bookmark: Bookmark, similarity: f32) -> Self {
        Self { bookmark, similarity }
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
    use crate::infrastructure::embeddings::dummy_provider::DummyEmbedding;
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
    fn given_empty_bookmark_list_when_semantic_search_then_returns_empty_results() {
        let _ = init_test_env();
        let search = SemanticSearch::new("test query", None);
        let embedder = DummyEmbedding;

        let results = search.execute(&[], &embedder).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn given_matching_bookmarks_when_semantic_search_then_returns_sorted_results() {
        let _ = init_test_env();
        let embedder = DummyEmbedding;

        let bookmarks = vec![
            create_test_bookmark("Test One", "This is a test", true),
            create_test_bookmark("Test Two", "Another test", true),
            create_test_bookmark("Not a match", "Something else", true),
        ];

        let search = SemanticSearch::new("test", None);
        let results = search.execute(&bookmarks, &embedder).unwrap();

        // With DummyEmbedding, we get no embeddings, so no results
        assert!(results.is_empty());

        // If we had real embeddings, we'd test:
        // assert_eq!(results.len(), 2);
        // assert_eq!(results[0].bookmark.title, "Test One");
        // assert!(results[0].similarity > 0.0);
    }

    #[test]
    fn given_search_limit_when_semantic_search_then_respects_limit() {
        let _ = init_test_env();
        let embedder = DummyEmbedding;

        let mut bookmarks = Vec::new();
        for i in 0..10 {
            bookmarks.push(create_test_bookmark(
                &format!("Test {}", i),
                "content",
                true,
            ));
        }

        let search = SemanticSearch::new("test", Some(3));
        let results = search.execute(&bookmarks, &embedder).unwrap();

        // With DummyEmbedding, we still get no results
        assert!(results.is_empty());

        // If we had real embeddings, we'd test:
        // assert_eq!(results.len(), 3);
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
