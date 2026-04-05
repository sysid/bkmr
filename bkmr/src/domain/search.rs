// src/domain/search.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::tag::Tag;
use std::collections::{HashMap, HashSet};

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
    pub similarity: f64,
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
    pub fn new(bookmark: Bookmark, similarity: f64) -> Self {
        Self {
            bookmark,
            similarity,
        }
    }

    /// Plain text display for semantic search results (used as skim search text)
    pub fn display(&self) -> String {
        let id = self.bookmark.id.unwrap_or(0);
        let title = &self.bookmark.title;
        let url = &self.bookmark.url;
        let binding = self.bookmark.formatted_tags();
        let tags_str = binding.trim_matches(',');
        let similarity = format!("{:.1}%", self.similarity * 100.0);

        let tags_display = if !tags_str.is_empty() {
            format!(" [{}]", tags_str)
        } else {
            String::new()
        };

        format!(
            "{}: {} <{}> ({}%) (default){}",
            id, title, url, similarity, tags_display
        )
    }
}

/// Search mode for hybrid search
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    /// Run both FTS and semantic search, fuse with RRF
    Hybrid,
    /// FTS only, skip semantic search
    Exact,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// A single item in a ranked result list, used as input to RRF fusion
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedResult {
    /// Bookmark ID
    pub bookmark_id: i32,
    /// 0-based rank position from the source engine
    pub rank: usize,
}

/// Represents a hybrid search query combining FTS and semantic search
#[derive(Debug, Clone)]
pub struct HybridSearch {
    /// The search query text
    pub query: String,
    /// All-of tag filter (pre-filter)
    pub tags_all: Option<HashSet<Tag>>,
    /// Exclude-all tag filter
    pub tags_all_not: Option<HashSet<Tag>>,
    /// Any-of tag filter
    pub tags_any: Option<HashSet<Tag>>,
    /// Exclude-any tag filter
    pub tags_any_not: Option<HashSet<Tag>>,
    /// Exact tag match filter
    pub tags_exact: Option<HashSet<Tag>>,
    /// Tag prefix filter
    pub tags_prefix: Option<HashSet<Tag>>,
    /// Max results to return (default: 10)
    pub limit: Option<usize>,
    /// Search mode: Hybrid (default) or Exact (FTS-only)
    pub mode: SearchMode,
}

impl HybridSearch {
    /// Create a new hybrid search with just a query
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            tags_all: None,
            tags_all_not: None,
            tags_any: None,
            tags_any_not: None,
            tags_exact: None,
            tags_prefix: None,
            limit: None,
            mode: SearchMode::default(),
        }
    }

    /// Returns true if any tag filters are set
    pub fn has_tag_filters(&self) -> bool {
        self.tags_all.is_some()
            || self.tags_all_not.is_some()
            || self.tags_any.is_some()
            || self.tags_any_not.is_some()
            || self.tags_exact.is_some()
            || self.tags_prefix.is_some()
    }

    /// Get the effective limit (default: 10)
    pub fn effective_limit(&self) -> usize {
        self.limit.unwrap_or(10)
    }

    /// Apply tag filters to a list of bookmarks, returning only those matching all criteria.
    /// Reuses the same tag matching logic as BookmarkQuery::apply_non_text_filters.
    pub fn apply_tag_filters<'a>(&self, bookmarks: &'a [Bookmark]) -> Vec<&'a Bookmark> {
        let mut filtered: Vec<&Bookmark> = bookmarks.iter().collect();

        if let Some(tags) = &self.tags_exact {
            if !tags.is_empty() {
                filtered.retain(|b| b.matches_exact_tags(tags));
            }
        }
        if let Some(tags) = &self.tags_all {
            if !tags.is_empty() {
                filtered.retain(|b| b.matches_all_tags(tags));
            }
        }
        if let Some(tags) = &self.tags_all_not {
            if !tags.is_empty() {
                filtered.retain(|b| !b.matches_all_tags(tags));
            }
        }
        if let Some(tags) = &self.tags_any {
            if !tags.is_empty() {
                filtered.retain(|b| b.matches_any_tag(tags));
            }
        }
        if let Some(tags) = &self.tags_any_not {
            if !tags.is_empty() {
                filtered.retain(|b| !b.matches_any_tag(tags));
            }
        }
        if let Some(prefixes) = &self.tags_prefix {
            if !prefixes.is_empty() {
                filtered.retain(|b| {
                    prefixes.iter().any(|prefix| {
                        let prefix_str = prefix.value();
                        b.tags.iter().any(|tag| tag.value().starts_with(prefix_str))
                    })
                });
            }
        }

        filtered
    }
}

/// Result of a hybrid search, including the bookmark and its RRF fusion score
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    /// The bookmark that matched
    pub bookmark: Bookmark,
    /// Combined RRF fusion score (higher = more relevant)
    pub rrf_score: f64,
}

impl HybridSearchResult {
    pub fn new(bookmark: Bookmark, rrf_score: f64) -> Self {
        Self { bookmark, rrf_score }
    }
}

/// Reciprocal Rank Fusion — merges ranked lists from multiple search engines
pub struct RrfFusion;

impl RrfFusion {
    /// Fuse two ranked result lists using RRF.
    ///
    /// Formula: `score(doc) = SUM(1 / (k + rank + 1))` where rank is 0-based.
    /// The `+1` converts 0-based rank to 1-based for the standard RRF formula.
    ///
    /// Returns `(bookmark_id, rrf_score)` pairs sorted descending by score, truncated to `limit`.
    pub fn fuse(
        fts_results: &[RankedResult],
        sem_results: &[RankedResult],
        k: f64,
        limit: usize,
    ) -> Vec<(i32, f64)> {
        let mut scores: HashMap<i32, f64> = HashMap::new();

        for result in fts_results {
            *scores.entry(result.bookmark_id).or_default() +=
                1.0 / (k + result.rank as f64 + 1.0);
        }
        for result in sem_results {
            *scores.entry(result.bookmark_id).or_default() +=
                1.0 / (k + result.rank as f64 + 1.0);
        }

        let mut scored: Vec<(i32, f64)> = scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
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

        let mut bookmark =
            Bookmark::new("https://example.com", title, content, tags).unwrap();

        bookmark.set_embeddable(has_embedding);
        bookmark
    }

    // === RRF Fusion Tests ===

    #[test]
    fn given_two_ranked_lists_when_fuse_then_boosted_score() {
        // Doc 1 appears in both lists → should get boosted score
        // Doc 2 appears only in FTS → single-engine score
        // Doc 3 appears only in semantic → single-engine score
        let fts = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
            RankedResult { bookmark_id: 2, rank: 1 },
        ];
        let sem = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
            RankedResult { bookmark_id: 3, rank: 1 },
        ];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 10);

        // Doc 1 should be first (appears in both lists)
        assert_eq!(results[0].0, 1);
        // Doc 1 score = 1/(60+0+1) + 1/(60+0+1) = 2/61
        let expected_score = 2.0 / 61.0;
        assert!((results[0].1 - expected_score).abs() < 1e-10);

        // Doc 2 and Doc 3 should have equal scores (both rank 1 in one list)
        let doc2_score = results.iter().find(|(id, _)| *id == 2).unwrap().1;
        let doc3_score = results.iter().find(|(id, _)| *id == 3).unwrap().1;
        assert!((doc2_score - doc3_score).abs() < 1e-10);
        // Each = 1/(60+1+1) = 1/62
        assert!((doc2_score - 1.0 / 62.0).abs() < 1e-10);
    }

    #[test]
    fn given_one_empty_list_when_fuse_then_single_engine_scores() {
        let fts = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
            RankedResult { bookmark_id: 2, rank: 1 },
        ];
        let sem: Vec<RankedResult> = vec![];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 10);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1);
        // Score = 1/(60+0+1) = 1/61
        assert!((results[0].1 - 1.0 / 61.0).abs() < 1e-10);
        assert_eq!(results[1].0, 2);
        // Score = 1/(60+1+1) = 1/62
        assert!((results[1].1 - 1.0 / 62.0).abs() < 1e-10);
    }

    #[test]
    fn given_tied_ranks_when_fuse_then_correct_scores() {
        // Two docs at same rank in different engines
        let fts = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
        ];
        let sem = vec![
            RankedResult { bookmark_id: 2, rank: 0 },
        ];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 10);

        assert_eq!(results.len(), 2);
        // Both should have equal scores: 1/(60+0+1) = 1/61
        assert!((results[0].1 - results[1].1).abs() < 1e-10);
        assert!((results[0].1 - 1.0 / 61.0).abs() < 1e-10);
    }

    #[test]
    fn given_k_constant_when_fuse_then_dampening_applied() {
        let fts = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
        ];
        let sem: Vec<RankedResult> = vec![];

        // With k=60: score = 1/61
        let results_k60 = RrfFusion::fuse(&fts, &sem, 60.0, 10);
        // With k=1: score = 1/2
        let results_k1 = RrfFusion::fuse(&fts, &sem, 1.0, 10);

        // Higher k dampens the score more
        assert!(results_k1[0].1 > results_k60[0].1);
        assert!((results_k60[0].1 - 1.0 / 61.0).abs() < 1e-10);
        assert!((results_k1[0].1 - 1.0 / 2.0).abs() < 1e-10);
    }

    #[test]
    fn given_limit_when_fuse_then_truncated() {
        let fts = vec![
            RankedResult { bookmark_id: 1, rank: 0 },
            RankedResult { bookmark_id: 2, rank: 1 },
            RankedResult { bookmark_id: 3, rank: 2 },
        ];
        let sem: Vec<RankedResult> = vec![];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 2);
        assert_eq!(results.len(), 2);
        // Top 2 by score
        assert_eq!(results[0].0, 1);
        assert_eq!(results[1].0, 2);
    }

    // === Existing Semantic Search Tests ===

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
