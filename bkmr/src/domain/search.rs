// src/domain/search.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::tag::Tag;
use std::collections::{HashMap, HashSet};

/// Search mode for hybrid search
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    /// Run both FTS and semantic search, fuse with RRF
    Hybrid,
    /// FTS only, skip semantic search
    Exact,
    /// Semantic search only, skip FTS
    Semantic,
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

    /// FTS side of the query: every whitespace-separated term becomes a quoted
    /// FTS5 string, so `c++`, `node.js`, `foo-bar` or `AND` match as text instead
    /// of failing as syntax. FTS5 power syntax belongs to `search`; in hybrid mode
    /// the same raw text is also embedded, where operators would only add noise.
    pub fn fts_query(&self) -> String {
        self.query
            .split_whitespace()
            .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" ")
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
    /// Cosine similarity to the query; `None` if the semantic search did not run or did not
    /// return this bookmark
    pub similarity: Option<f64>,
    /// Whether the keyword (FTS) search returned this bookmark; `None` if it did not run
    pub keyword_match: Option<bool>,
}

impl HybridSearchResult {
    pub fn new(
        bookmark: Bookmark,
        rrf_score: f64,
        similarity: Option<f64>,
        keyword_match: Option<bool>,
    ) -> Self {
        Self {
            bookmark,
            rrf_score,
            similarity,
            keyword_match,
        }
    }
}

/// Cosine similarity from the L2 distance between two unit vectors: `1 - d²/2`.
///
/// Exact only for unit vectors under the L2 metric. Both hold for stored embeddings: FastEmbed
/// normalizes every text embedding, and the `vec0` table uses its default metric, L2, which
/// returns the true Euclidean distance (not its square).
pub fn cosine_from_l2(distance: f64) -> f64 {
    1.0 - distance * distance / 2.0
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
            *scores.entry(result.bookmark_id).or_default() += 1.0 / (k + result.rank as f64 + 1.0);
        }
        for result in sem_results {
            *scores.entry(result.bookmark_id).or_default() += 1.0 / (k + result.rank as f64 + 1.0);
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

    // === RRF Fusion Tests ===

    #[test]
    fn given_two_ranked_lists_when_fuse_then_boosted_score() {
        // Doc 1 appears in both lists → should get boosted score
        // Doc 2 appears only in FTS → single-engine score
        // Doc 3 appears only in semantic → single-engine score
        let fts = vec![
            RankedResult {
                bookmark_id: 1,
                rank: 0,
            },
            RankedResult {
                bookmark_id: 2,
                rank: 1,
            },
        ];
        let sem = vec![
            RankedResult {
                bookmark_id: 1,
                rank: 0,
            },
            RankedResult {
                bookmark_id: 3,
                rank: 1,
            },
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
            RankedResult {
                bookmark_id: 1,
                rank: 0,
            },
            RankedResult {
                bookmark_id: 2,
                rank: 1,
            },
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
        let fts = vec![RankedResult {
            bookmark_id: 1,
            rank: 0,
        }];
        let sem = vec![RankedResult {
            bookmark_id: 2,
            rank: 0,
        }];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 10);

        assert_eq!(results.len(), 2);
        // Both should have equal scores: 1/(60+0+1) = 1/61
        assert!((results[0].1 - results[1].1).abs() < 1e-10);
        assert!((results[0].1 - 1.0 / 61.0).abs() < 1e-10);
    }

    #[test]
    fn given_k_constant_when_fuse_then_dampening_applied() {
        let fts = vec![RankedResult {
            bookmark_id: 1,
            rank: 0,
        }];
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
            RankedResult {
                bookmark_id: 1,
                rank: 0,
            },
            RankedResult {
                bookmark_id: 2,
                rank: 1,
            },
            RankedResult {
                bookmark_id: 3,
                rank: 2,
            },
        ];
        let sem: Vec<RankedResult> = vec![];

        let results = RrfFusion::fuse(&fts, &sem, 60.0, 2);
        assert_eq!(results.len(), 2);
        // Top 2 by score
        assert_eq!(results[0].0, 1);
        assert_eq!(results[1].0, 2);
    }

    #[test]
    fn given_l2_distance_of_unit_vectors_when_converted_then_returns_cosine_similarity() {
        // identical, orthogonal and opposite unit vectors
        assert!((cosine_from_l2(0.0) - 1.0).abs() < 1e-12);
        assert!(cosine_from_l2(2f64.sqrt()).abs() < 1e-12);
        assert!((cosine_from_l2(2.0) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn given_terms_with_fts_operators_when_fts_query_then_each_term_is_quoted() {
        let search = HybridSearch::new("c++ node.js foo-bar title:rust AND deploy*");

        assert_eq!(
            search.fts_query(),
            r#""c++" "node.js" "foo-bar" "title:rust" "AND" "deploy*""#
        );
    }

    #[test]
    fn given_term_containing_double_quote_when_fts_query_then_quote_is_escaped() {
        let search = HybridSearch::new(r#"say"hi"#);

        assert_eq!(search.fts_query(), r#""say""hi""#);
    }

    #[test]
    fn given_literal_fts_query_when_rendered_then_semantic_query_is_unchanged() {
        let search = HybridSearch::new("  c++   tips ");

        assert_eq!(search.fts_query(), r#""c++" "tips""#);
        assert_eq!(search.query, "  c++   tips ");
    }
}
