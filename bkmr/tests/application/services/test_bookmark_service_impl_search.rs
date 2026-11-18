// src/application/services/bookmark_service_impl_integration_tests.rs
use std::collections::HashSet;
use std::sync::Arc;

use bkmr::application::services::bookmark_service::BookmarkService;
use bkmr::application::BookmarkServiceImpl;
use bkmr::domain::repositories::query::{BookmarkQuery, SortCriteria, SortDirection, SortField};
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::embeddings::DummyEmbedding;
use bkmr::infrastructure::repositories::json_import_repository::JsonImportRepository;
use bkmr::infrastructure::repositories::null_vector_repository::NullVectorRepository;
use bkmr::util::testing::{init_test_env, setup_test_db, EnvGuard};

// Helper function to create a test service
fn create_test_service() -> impl BookmarkService {
    let repository = setup_test_db();
    let arc_repository = Arc::new(repository);
    let embedder = Arc::new(DummyEmbedding);
    let vector_repository = Arc::new(NullVectorRepository);
    BookmarkServiceImpl::new(
        arc_repository,
        embedder,
        vector_repository,
        Arc::new(JsonImportRepository::new()),
    )
}

// Helper function to parse tag strings into HashSet<Tag>
fn parse_tags(tag_str: &str) -> HashSet<Tag> {
    Tag::parse_tags(tag_str).unwrap_or_else(|_| HashSet::new())
}

// Create an adapter function to maintain backward compatibility for tests
fn search_bookmarks(
    service: &impl BookmarkService,
    text_query: Option<&str>,
    tags_exact: Option<&HashSet<Tag>>,
    tags_all: Option<&HashSet<Tag>>,
    tags_all_not: Option<&HashSet<Tag>>,
    tags_any: Option<&HashSet<Tag>>,
    tags_any_not: Option<&HashSet<Tag>>,
    tags_prefix: Option<&HashSet<Tag>>,
    sort_direction: SortDirection,
    limit: Option<usize>,
) -> Result<Vec<bkmr::domain::bookmark::Bookmark>, bkmr::application::error::ApplicationError> {
    // Create a BookmarkQuery object
    let query = BookmarkQuery::new()
        .with_text_query(text_query)
        .with_tags_exact(tags_exact)
        .with_tags_all(tags_all)
        .with_tags_all_not(tags_all_not)
        .with_tags_any(tags_any)
        .with_tags_any_not(tags_any_not)
        .with_tags_prefix(tags_prefix)
        .with_sort(SortCriteria::new(SortField::Modified, sort_direction));

    let query_with_limit = if let Some(lim) = limit {
        query.with_limit(Option::from(lim))
    } else {
        query
    };

    service.search_bookmarks(&query_with_limit)
}

// Adds a bookmark without web fetch and returns its id
fn add(service: &impl BookmarkService, url: &str, title: &str, tags: &str) -> i32 {
    service
        .add_bookmark(
            url,
            Some(title),
            None,
            Some(&parse_tags(tags)),
            false,
            Some(true),
            None,
        )
        .unwrap()
        .id
        .unwrap()
}

fn ids(bookmarks: &[bkmr::domain::bookmark::Bookmark]) -> HashSet<i32> {
    bookmarks.iter().filter_map(|b| b.id).collect()
}

#[test]
fn given_complex_tag_combinations_when_search_bookmarks_then_returns_correct_results() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let aaa_bbb = add(
        &service,
        "https://aaa-bbb.example.com",
        "aaa bbb",
        "aaa,bbb",
    );
    let aaa_bbb_ccc = add(
        &service,
        "https://aaa-bbb-ccc.example.com",
        "aaa bbb ccc",
        "aaa,bbb,ccc",
    );
    let ccc_yyy = add(
        &service,
        "https://ccc-yyy.example.com",
        "ccc yyy",
        "ccc,yyy",
    );
    let xxx = add(&service, "https://xxx.example.com", "xxx", "xxx");
    let _aaa = add(&service, "https://aaa.example.com", "aaa", "aaa");

    // Case 1: All tags must be "aaa" AND "bbb"
    let tags_all = parse_tags("aaa,bbb");
    let results = search_bookmarks(
        &service,
        None,
        None,
        Some(&tags_all),
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results), HashSet::from([aaa_bbb, aaa_bbb_ccc]));

    // Case 2: Any tag must be "xxx" OR "yyy"
    let tags_any = parse_tags("xxx,yyy");
    let results_any = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        Some(&tags_any),
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results_any), HashSet::from([ccc_yyy, xxx]));

    // Case 3: Complex - (has "aaa" AND "bbb") but NOT "ccc"
    let tags_all = parse_tags("aaa,bbb");
    let tags_all_not = parse_tags("ccc");
    let results_complex = search_bookmarks(
        &service,
        None,
        None,
        Some(&tags_all),
        Some(&tags_all_not),
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results_complex), HashSet::from([aaa_bbb]));
}

#[test]
fn given_text_query_with_tag_filtering_when_search_bookmarks_then_combines_filters_correctly() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let text_and_tag = add(&service, "https://one.example.com", "TEST entry one", "aaa");
    let _text_only = add(&service, "https://two.example.com", "TEST entry two", "bbb");
    let tag_only = add(&service, "https://three.example.com", "Other entry", "aaa");

    // Search for "TEST" in text and filter by tag "aaa"
    let query = "TEST";
    let tags_all = parse_tags("aaa");

    let results = search_bookmarks(
        &service,
        Some(query),
        None,
        Some(&tags_all),
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    // Assert both text and tag filters were applied
    assert_eq!(ids(&results), HashSet::from([text_and_tag]));

    // Compare with results from just tag filter
    let tag_only_results = search_bookmarks(
        &service,
        None,
        None,
        Some(&tags_all),
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(
        ids(&tag_only_results),
        HashSet::from([text_and_tag, tag_only])
    );
}

#[test]
fn given_exact_tag_match_when_search_bookmarks_then_returns_exact_matches_only() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let exact = add(&service, "https://exact.example.com", "exact", "aaa,bbb");
    let superset = add(
        &service,
        "https://superset.example.com",
        "superset",
        "aaa,bbb,ccc",
    );
    let _subset = add(&service, "https://subset.example.com", "subset", "aaa");

    let tags_exact = parse_tags("aaa,bbb");

    let results = search_bookmarks(
        &service,
        None,
        Some(&tags_exact),
        None,
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    // Each bookmark must have EXACTLY these tags, no more, no less
    assert_eq!(ids(&results), HashSet::from([exact]));

    // Compare with "all tags" (which allows additional tags)
    let all_tags_results = search_bookmarks(
        &service,
        None,
        None,
        Some(&tags_exact),
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&all_tags_results), HashSet::from([exact, superset]));
}

#[test]
fn given_tag_prefix_when_search_bookmarks_then_returns_matching_prefixed_tags() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let aaa = add(&service, "https://aaa.example.com", "aaa", "aaa");
    let abc = add(&service, "https://abc.example.com", "abc", "abc,zzz");
    let _bbb = add(&service, "https://bbb.example.com", "bbb", "bbb");
    let _untagged = add(&service, "https://untagged.example.com", "untagged", "");

    let mut prefix_tags = HashSet::new();
    prefix_tags.insert(Tag::new("a").unwrap());

    let results = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&prefix_tags),
        SortDirection::Descending,
        None,
    )
    .unwrap();

    // Only bookmarks with at least one tag starting with 'a'
    assert_eq!(ids(&results), HashSet::from([aaa, abc]));
}

#[test]
fn given_negated_tag_filters_when_search_bookmarks_then_excludes_correctly() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let _with_ccc = add(
        &service,
        "https://with-ccc.example.com",
        "With ccc",
        "aaa,ccc",
    );
    let without_ccc = add(
        &service,
        "https://without-ccc.example.com",
        "Without ccc",
        "aaa,bbb",
    );
    let untagged = add(&service, "https://untagged.example.com", "Untagged", "");

    // Exclude bookmarks with tag "ccc"
    let tags_any_not = parse_tags("ccc");

    let results = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        Some(&tags_any_not),
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    // Assert: only the bookmarks without "ccc" remain, including the untagged one
    assert_eq!(ids(&results), HashSet::from([without_ccc, untagged]));
}

#[test]
fn given_sort_direction_and_limit_when_search_bookmarks_then_respects_ordering_and_limits() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    for i in 1..=5 {
        add(
            &service,
            &format!("https://sort-{i}.example.com"),
            "sort",
            "",
        );
    }

    // Get all bookmarks sorted descending (newest first)
    let desc_results = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();
    assert_eq!(desc_results.len(), 5);

    // Get all bookmarks sorted ascending (oldest first)
    let _ = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        SortDirection::Ascending,
        None,
    )
    .unwrap();

    // todo: fix this test
    // NOT WORKING: test db all same timestamp
    // The orders should be reversed
    // if !desc_results.is_empty() && !asc_results.is_empty() {
    //     assert_ne!(
    //         desc_results.first().unwrap().updated_at,
    //         asc_results.first().unwrap().updated_at,
    //         "First items should differ between sort orders"
    //     );
    // }

    // Test with limit
    let limit = 3;
    let limited_results = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        Some(limit),
    )
    .unwrap();

    // Should respect the limit
    assert_eq!(limited_results.len(), limit);

    // Limited results should match the first 'limit' items from unlimited results
    for (limited, unlimited) in limited_results.iter().zip(desc_results.iter()) {
        assert_eq!(limited.id, unlimited.id);
    }
}

#[test]
fn given_highly_specific_filter_combination_when_search_bookmarks_then_filters_apply_in_correct_order(
) {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let match_one = add(&service, "https://h1.example.com", "TEST one", "aaa,bbb");
    let _has_xxx = add(
        &service,
        "https://h2.example.com",
        "TEST two",
        "aaa,bbb,xxx",
    );
    let match_three = add(&service, "https://h3.example.com", "TEST three", "aaa,bbb");
    let match_four = add(&service, "https://h4.example.com", "TEST four", "aaa,bbb");
    let _no_text = add(&service, "https://h5.example.com", "Other", "aaa,bbb");
    let _missing_bbb = add(&service, "https://h6.example.com", "TEST six", "aaa");

    // Create a complex filter:
    // - Text contains "TEST"
    // - Must have tags "aaa" AND "bbb"
    // - Must NOT have tag "xxx"
    // - Limit to top 2 results (3 bookmarks match, so the limit applies)

    let query = "TEST";
    let tags_all = parse_tags("aaa,bbb");
    let tags_any_not = parse_tags("xxx");
    let limit = 2;

    let results = search_bookmarks(
        &service,
        Some(query),
        None,
        Some(&tags_all),
        None,
        None,
        Some(&tags_any_not),
        None,
        SortDirection::Descending,
        Some(limit),
    )
    .unwrap();

    // Verify all filters and the limit were applied
    assert_eq!(results.len(), limit);
    assert!(ids(&results).is_subset(&HashSet::from([match_one, match_three, match_four])));

    // Also verify we get the same results with filters applied in different order
    // First apply text filter and tag filters
    let intermediate_results = search_bookmarks(
        &service,
        Some(query),
        None,
        Some(&tags_all),
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    // Then manually apply the negative tag filter
    let manually_filtered: Vec<_> = intermediate_results
        .into_iter()
        .filter(|b| !b.tags.contains(&Tag::new("xxx").unwrap()))
        .take(limit)
        .collect();

    assert_eq!(
        ids(&results),
        ids(&manually_filtered),
        "Filters should be applied in a consistent manner"
    );
}

#[test]
fn given_empty_filters_when_search_bookmarks_then_returns_expected_defaults() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let tagged = add(&service, "https://tagged.example.com", "tagged", "aaa");
    let other_tagged = add(
        &service,
        "https://other-tagged.example.com",
        "other tagged",
        "bbb,ccc",
    );
    let untagged = add(&service, "https://untagged.example.com", "untagged", "");
    let all_ids = HashSet::from([tagged, other_tagged, untagged]);

    // Case 1: All filters None except sort (should return all bookmarks sorted)
    let results_default = search_bookmarks(
        &service,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results_default), all_ids);
    let all_bookmarks = service
        .get_all_bookmarks(Some(SortDirection::Descending), None)
        .unwrap();
    assert_eq!(ids(&all_bookmarks), all_ids);

    // Case 2: Empty text query (should be treated as no text query)
    let results_empty_query = search_bookmarks(
        &service,
        Some(""),
        None,
        None,
        None,
        None,
        None,
        None,
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results_empty_query), all_ids);

    // Case 3: Empty tag sets (should be treated as no tag filter)
    let empty_tags = HashSet::new();
    let results_empty_tags = search_bookmarks(
        &service,
        None,
        Some(&empty_tags),
        Some(&empty_tags),
        Some(&empty_tags),
        Some(&empty_tags),
        Some(&empty_tags),
        Some(&empty_tags),
        SortDirection::Descending,
        None,
    )
    .unwrap();

    assert_eq!(ids(&results_empty_tags), all_ids);
}

// --- hybrid_search: vector repository failure must surface ---

/// Vector store whose availability probe fails, as with a locked or corrupt database.
#[derive(Debug)]
struct FailingVectorRepository;

impl bkmr::domain::repositories::vector_repository::VectorRepository for FailingVectorRepository {
    fn init_vec_table(&self, _dimensions: usize) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn upsert_embedding(&self, _id: i32, _e: &[f32]) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn delete_embedding(&self, _id: i32) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn search_nearest(
        &self,
        _q: &[f32],
        _limit: usize,
    ) -> bkmr::domain::error::DomainResult<Vec<(i32, f64)>> {
        unimplemented!()
    }
    fn has_embeddings(&self) -> bkmr::domain::error::DomainResult<bool> {
        Err(bkmr::domain::error::DomainError::BookmarkOperationFailed(
            "vec_bookmarks probe failed: database is locked".to_string(),
        ))
    }
    fn get_dimensions(&self) -> bkmr::domain::error::DomainResult<Option<usize>> {
        unimplemented!()
    }
    fn clear_all(&self) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn get_embedded_ids(&self) -> bkmr::domain::error::DomainResult<HashSet<i32>> {
        unimplemented!()
    }
    fn search_nearest_filtered(
        &self,
        _q: &[f32],
        _limit: usize,
        _filter_ids: Option<&HashSet<i32>>,
    ) -> bkmr::domain::error::DomainResult<Vec<(i32, f64)>> {
        unimplemented!()
    }
}

/// Embedder with real dimensions so hybrid search does not skip the vector branch.
#[derive(Debug)]
struct FixedDimensionEmbedder;

impl bkmr::domain::embedding::Embedder for FixedDimensionEmbedder {
    fn embed_document(&self, _text: &str) -> bkmr::domain::error::DomainResult<Option<Vec<f32>>> {
        unimplemented!()
    }
    fn embed_query(&self, _text: &str) -> bkmr::domain::error::DomainResult<Option<Vec<f32>>> {
        unimplemented!()
    }
    fn dimensions(&self) -> usize {
        384
    }
}

#[test]
fn given_vector_repository_fails_when_hybrid_search_then_error_is_returned() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = BookmarkServiceImpl::new(
        Arc::new(setup_test_db()),
        Arc::new(FixedDimensionEmbedder),
        Arc::new(FailingVectorRepository),
        Arc::new(JsonImportRepository::new()),
    );

    let result = service.hybrid_search(&bkmr::domain::search::HybridSearch::new("rust"));

    let error = result.expect_err("vector store failure must not degrade to FTS-only results");
    assert!(
        error.to_string().contains("database is locked"),
        "error should carry the store's cause, got: {error}"
    );
}

// --- hybrid_search: query text is literal on the FTS side ---

#[test]
fn given_query_with_fts_operator_characters_when_hybrid_search_then_matches_literally() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();
    let id = add(
        &service,
        "https://literal-fts.example.com",
        "C++ and node.js: foo-bar tips",
        "",
    );

    for query in ["c++", "node.js", "foo-bar", "AND", "tips:"] {
        let results = service
            .hybrid_search(&bkmr::domain::search::HybridSearch::new(query))
            .unwrap_or_else(|e| panic!("query {query:?} must not be parsed as FTS5 syntax: {e}"))
            .hits;

        assert!(
            results.iter().any(|r| r.bookmark.id == Some(id)),
            "query {query:?} should find the bookmark"
        );
    }
}

// --- hybrid_search: blank query is a validation error ---

#[test]
fn given_blank_query_when_hybrid_search_then_validation_error() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    for query in ["", "   "] {
        let error = service
            .hybrid_search(&bkmr::domain::search::HybridSearch::new(query))
            .expect_err("blank query must be rejected before reaching FTS5");

        assert!(
            matches!(
                error,
                bkmr::application::error::ApplicationError::Validation(_)
            ),
            "query {query:?} should fail validation, got: {error}"
        );
    }
}

// --- hybrid_search: semantic mode, similarity and keyword_match ---

/// Vector store that returns fixed (id, L2 distance) pairs, nearest first, honouring the ID filter.
#[derive(Debug)]
struct FixedNeighboursVectorRepository {
    neighbours: Vec<(i32, f64)>,
}

impl bkmr::domain::repositories::vector_repository::VectorRepository
    for FixedNeighboursVectorRepository
{
    fn init_vec_table(&self, _dimensions: usize) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn upsert_embedding(&self, _id: i32, _e: &[f32]) -> bkmr::domain::error::DomainResult<()> {
        Ok(())
    }
    fn delete_embedding(&self, _id: i32) -> bkmr::domain::error::DomainResult<()> {
        Ok(())
    }
    fn search_nearest(
        &self,
        _q: &[f32],
        _limit: usize,
    ) -> bkmr::domain::error::DomainResult<Vec<(i32, f64)>> {
        unimplemented!()
    }
    fn has_embeddings(&self) -> bkmr::domain::error::DomainResult<bool> {
        Ok(!self.neighbours.is_empty())
    }
    fn get_dimensions(&self) -> bkmr::domain::error::DomainResult<Option<usize>> {
        Ok(Some(384))
    }
    fn clear_all(&self) -> bkmr::domain::error::DomainResult<()> {
        unimplemented!()
    }
    fn get_embedded_ids(&self) -> bkmr::domain::error::DomainResult<HashSet<i32>> {
        Ok(self.neighbours.iter().map(|(id, _)| *id).collect())
    }
    fn search_nearest_filtered(
        &self,
        _q: &[f32],
        limit: usize,
        filter_ids: Option<&HashSet<i32>>,
    ) -> bkmr::domain::error::DomainResult<Vec<(i32, f64)>> {
        Ok(self
            .neighbours
            .iter()
            .filter(|(id, _)| filter_ids.is_none_or(|f| f.contains(id)))
            .take(limit)
            .copied()
            .collect())
    }
}

/// Embedder with real dimensions that embeds every query to the same vector.
#[derive(Debug)]
struct ConstantQueryEmbedder;

impl bkmr::domain::embedding::Embedder for ConstantQueryEmbedder {
    fn embed_document(&self, _text: &str) -> bkmr::domain::error::DomainResult<Option<Vec<f32>>> {
        Ok(None)
    }
    fn embed_query(&self, _text: &str) -> bkmr::domain::error::DomainResult<Option<Vec<f32>>> {
        Ok(Some(vec![0.0; 384]))
    }
    fn dimensions(&self) -> usize {
        384
    }
}

/// Service over a fresh test DB with three bookmarks, returned as (service, keyword_only,
/// both, meaning_only). Only `keyword_only` and `both` contain the word "kubernetes";
/// the vector store returns `both` (distance 0.6) and `meaning_only` (distance 0.8).
fn service_with_keyword_and_meaning_hits() -> (impl BookmarkService, i32, i32, i32) {
    let repository = Arc::new(setup_test_db());
    // Add with the null store so adding does not depend on the neighbour ids
    let seeding = BookmarkServiceImpl::new(
        repository.clone(),
        Arc::new(DummyEmbedding),
        Arc::new(NullVectorRepository),
        Arc::new(JsonImportRepository::new()),
    );
    let keyword_only = add(
        &seeding,
        "https://k.example.com",
        "kubernetes cheat sheet",
        "ops",
    );
    let both = add(
        &seeding,
        "https://b.example.com",
        "kubernetes cluster setup",
        "ops",
    );
    let meaning_only = add(
        &seeding,
        "https://m.example.com",
        "container orchestration",
        "infra",
    );

    let service = BookmarkServiceImpl::new(
        repository,
        Arc::new(ConstantQueryEmbedder),
        Arc::new(FixedNeighboursVectorRepository {
            neighbours: vec![(both, 0.6), (meaning_only, 0.8)],
        }),
        Arc::new(JsonImportRepository::new()),
    );
    (service, keyword_only, both, meaning_only)
}

fn search_with_mode(
    service: &impl BookmarkService,
    mode: bkmr::domain::search::SearchMode,
) -> Vec<bkmr::domain::search::HybridSearchResult> {
    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.mode = mode;
    service.hybrid_search(&search).unwrap().hits
}

fn hit(
    results: &[bkmr::domain::search::HybridSearchResult],
    id: i32,
) -> &bkmr::domain::search::HybridSearchResult {
    results
        .iter()
        .find(|r| r.bookmark.id == Some(id))
        .unwrap_or_else(|| panic!("bookmark {id} missing from results"))
}

#[test]
fn given_semantic_mode_when_hybrid_search_then_returns_only_vector_hits_with_cosine_similarity() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let (service, keyword_only, both, meaning_only) = service_with_keyword_and_meaning_hits();

    let results = search_with_mode(&service, bkmr::domain::search::SearchMode::Semantic);

    // The keyword-only bookmark has no vector, so semantic mode cannot find it
    let found: Vec<i32> = results.iter().filter_map(|r| r.bookmark.id).collect();
    assert_eq!(
        found,
        vec![both, meaning_only],
        "nearest first, no keyword hits"
    );
    assert!(!found.contains(&keyword_only));
    // cosine = 1 - d²/2 for unit vectors
    assert!((hit(&results, both).similarity.unwrap() - 0.82).abs() < 1e-9);
    assert!((hit(&results, meaning_only).similarity.unwrap() - 0.68).abs() < 1e-9);
    // Keyword search did not run, so there is no claim either way
    assert!(results.iter().all(|r| r.keyword_match.is_none()));
}

#[test]
fn given_semantic_mode_and_tag_filter_when_hybrid_search_then_only_filtered_bookmarks_are_returned()
{
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let (service, _keyword_only, both, _meaning_only) = service_with_keyword_and_meaning_hits();

    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.mode = bkmr::domain::search::SearchMode::Semantic;
    search.tags_all = Some(parse_tags("ops"));
    let results = service.hybrid_search(&search).unwrap().hits;

    let found: Vec<i32> = results.iter().filter_map(|r| r.bookmark.id).collect();
    assert_eq!(found, vec![both]);
}

#[test]
fn given_hybrid_mode_when_hybrid_search_then_each_hit_says_whether_keyword_and_meaning_matched() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let (service, keyword_only, both, meaning_only) = service_with_keyword_and_meaning_hits();

    let results = search_with_mode(&service, bkmr::domain::search::SearchMode::Hybrid);

    let keyword = hit(&results, keyword_only);
    assert_eq!(keyword.keyword_match, Some(true));
    assert_eq!(keyword.similarity, None);

    let both_hit = hit(&results, both);
    assert_eq!(both_hit.keyword_match, Some(true));
    assert!((both_hit.similarity.unwrap() - 0.82).abs() < 1e-9);

    let meaning = hit(&results, meaning_only);
    assert_eq!(meaning.keyword_match, Some(false));
    assert!((meaning.similarity.unwrap() - 0.68).abs() < 1e-9);
}

#[test]
fn given_exact_mode_when_hybrid_search_then_every_hit_is_a_keyword_match_without_similarity() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let (service, keyword_only, both, meaning_only) = service_with_keyword_and_meaning_hits();

    let results = search_with_mode(&service, bkmr::domain::search::SearchMode::Exact);

    let found: HashSet<i32> = results.iter().filter_map(|r| r.bookmark.id).collect();
    assert_eq!(found, HashSet::from([keyword_only, both]));
    assert!(!found.contains(&meaning_only));
    for result in &results {
        assert_eq!(result.keyword_match, Some(true));
        assert_eq!(result.similarity, None);
    }
}

#[test]
fn given_keyword_hit_ranked_below_the_fts_cut_when_hybrid_search_then_still_a_keyword_match() {
    use bkmr::domain::repositories::repository::BookmarkRepository;

    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let repository = Arc::new(setup_test_db());
    let seeding = BookmarkServiceImpl::new(
        repository.clone(),
        Arc::new(DummyEmbedding),
        Arc::new(NullVectorRepository),
        Arc::new(JsonImportRepository::new()),
    );
    for i in 0..25 {
        add(
            &seeding,
            &format!("https://k{i}.example.com"),
            &format!("kubernetes note {i}"),
            "ops",
        );
    }
    // limit 5 keeps the 20 best keyword hits for fusion; the vector store returns the rest
    let below_cut: Vec<i32> = repository
        .get_bookmarks_fts_ranked("\"kubernetes\"", None)
        .unwrap()
        .into_iter()
        .skip(20)
        .map(|r| r.bookmark_id)
        .collect();
    assert_eq!(below_cut.len(), 5);
    let service = BookmarkServiceImpl::new(
        repository,
        Arc::new(ConstantQueryEmbedder),
        Arc::new(FixedNeighboursVectorRepository {
            neighbours: below_cut.iter().map(|id| (*id, 0.6)).collect(),
        }),
        Arc::new(JsonImportRepository::new()),
    );

    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.limit = Some(5);
    let results = service.hybrid_search(&search).unwrap().hits;

    assert!(
        results
            .iter()
            .any(|r| below_cut.contains(&r.bookmark.id.unwrap())),
        "a bookmark below the cut must reach the results, or this test proves nothing"
    );
    // Every bookmark contains the word, whatever its keyword rank
    for result in &results {
        assert_eq!(
            result.keyword_match,
            Some(true),
            "bookmark {:?}",
            result.bookmark.id
        );
    }
}

// --- hybrid_search: reports when semantic search had no vectors to search ---

#[test]
fn given_no_stored_vectors_when_semantic_search_then_outcome_reports_missing_embeddings() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = BookmarkServiceImpl::new(
        Arc::new(setup_test_db()),
        Arc::new(ConstantQueryEmbedder),
        Arc::new(NullVectorRepository),
        Arc::new(JsonImportRepository::new()),
    );

    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.mode = bkmr::domain::search::SearchMode::Semantic;
    let outcome = service.hybrid_search(&search).unwrap();

    assert!(outcome.hits.is_empty());
    assert!(outcome.missing_embeddings);
}

#[test]
fn given_stored_vectors_when_semantic_search_then_outcome_does_not_report_missing_embeddings() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let (service, _keyword_only, _both, _meaning_only) = service_with_keyword_and_meaning_hits();

    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.mode = bkmr::domain::search::SearchMode::Semantic;
    let outcome = service.hybrid_search(&search).unwrap();

    assert!(!outcome.hits.is_empty());
    assert!(!outcome.missing_embeddings);
}

#[test]
fn given_exact_mode_without_vectors_when_hybrid_search_then_missing_embeddings_is_not_reported() {
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = BookmarkServiceImpl::new(
        Arc::new(setup_test_db()),
        Arc::new(ConstantQueryEmbedder),
        Arc::new(NullVectorRepository),
        Arc::new(JsonImportRepository::new()),
    );

    let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
    search.mode = bkmr::domain::search::SearchMode::Exact;
    let outcome = service.hybrid_search(&search).unwrap();

    // Exact mode never wanted vectors, so their absence is not news
    assert!(!outcome.missing_embeddings);
}

// --- hybrid_search: embedding model and stored vectors disagree on dimensions ---

#[test]
fn given_stored_vectors_of_other_dimension_when_hybrid_search_then_error_names_backfill_force() {
    use bkmr::domain::repositories::vector_repository::VectorRepository;
    use bkmr::infrastructure::repositories::sqlite::register_sqlite_vec;
    use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;

    let _env = init_test_env();
    let _guard = EnvGuard::new();
    // Vectors stored by a 768-dimension model; the current embedder produces 384
    let dir = tempfile::TempDir::new().unwrap();
    let vector_db = dir.path().join("vectors.db");
    register_sqlite_vec();
    let vectors = SqliteVectorRepository::new(vector_db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(768).unwrap();
    vectors.upsert_embedding(1, &vec![0.1; 768]).unwrap();
    let service = BookmarkServiceImpl::new(
        Arc::new(setup_test_db()),
        Arc::new(ConstantQueryEmbedder),
        Arc::new(vectors),
        Arc::new(JsonImportRepository::new()),
    );

    for mode in [
        bkmr::domain::search::SearchMode::Hybrid,
        bkmr::domain::search::SearchMode::Semantic,
    ] {
        let mut search = bkmr::domain::search::HybridSearch::new("kubernetes");
        search.mode = mode.clone();
        let error = service
            .hybrid_search(&search)
            .expect_err("a dimension mismatch must not pass silently");

        assert!(
            error.to_string().contains("bkmr backfill --force"),
            "{mode:?}: error should say how to fix it, got: {error}"
        );
    }
}
