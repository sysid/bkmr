// src/application/services/bookmark_service_impl_integration_tests.rs
use std::collections::HashSet;
use std::sync::Arc;

use bkmr::application::services::bookmark_service::BookmarkService;
use bkmr::application::BookmarkServiceImpl;
use bkmr::domain::repositories::query::SortDirection;
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::embeddings::DummyEmbedding;
use bkmr::infrastructure::repositories::json_import_repository::JsonImportRepository;
use bkmr::util::testing::{init_test_env, setup_test_db, EnvGuard};
use serial_test::serial;

// Helper function to create a test service
fn create_test_service() -> impl BookmarkService {
    let repository = setup_test_db();
    let arc_repository = Arc::new(repository);
    let embedder = Arc::new(DummyEmbedding);
    BookmarkServiceImpl::new(
        arc_repository,
        embedder,
        Arc::new(JsonImportRepository::new()),
    )
}

// Helper function to parse tag strings into HashSet<Tag>
fn parse_tags(tag_str: &str) -> HashSet<Tag> {
    Tag::parse_tags(tag_str).unwrap_or_else(|_| HashSet::new())
}

#[test]
#[serial]
fn given_complex_tag_combinations_when_search_bookmarks_then_returns_correct_results() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Based on up.sql, we know these tags exist: aaa, bbb, ccc, xxx, yyy
    let _ = parse_tags("aaa,bbb,ccc,xxx,yyy");

    // Get all bookmarks to understand the dataset
    let all_bookmarks = service.get_all_bookmarks(None, None).unwrap();
    println!("Total bookmarks in test DB: {}", all_bookmarks.len());

    // Case 1: All tags must be "aaa" AND "bbb"
    let tags_all = parse_tags("aaa,bbb");
    let results = service
        .search_bookmarks(
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

    // Assert all results have both tags
    for bookmark in &results {
        assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));
        assert!(bookmark.tags.contains(&Tag::new("bbb").unwrap()));
    }

    // Case 2: Any tag must be "xxx" OR "yyy"
    let tags_any = parse_tags("xxx,yyy");
    let results_any = service
        .search_bookmarks(
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

    // Assert each result has at least one of the tags
    for bookmark in &results_any {
        assert!(
            bookmark.tags.contains(&Tag::new("xxx").unwrap())
                || bookmark.tags.contains(&Tag::new("yyy").unwrap())
        );
    }

    // Case 3: Complex - (has "aaa" AND "bbb") but NOT "ccc"
    let tags_all = parse_tags("aaa,bbb");
    let tags_all_not = parse_tags("ccc");
    let results_complex = service
        .search_bookmarks(
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

    // Assert correct filtering
    for bookmark in &results_complex {
        assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));
        assert!(bookmark.tags.contains(&Tag::new("bbb").unwrap()));
        assert!(!bookmark.tags.contains(&Tag::new("ccc").unwrap()));
    }
}

#[test]
#[serial]
fn given_text_query_with_tag_filtering_when_search_bookmarks_then_combines_filters_correctly() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Search for "TEST" in text and filter by tag "aaa"
    let query = "TEST";
    let tags_all = parse_tags("aaa");

    let results = service
        .search_bookmarks(
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
    for bookmark in &results {
        // Should have the tag
        assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));

        // Should match the text query
        let text_match = bookmark
            .title
            .to_lowercase()
            .contains(&query.to_lowercase())
            || bookmark
                .description
                .to_lowercase()
                .contains(&query.to_lowercase())
            || bookmark.url.to_lowercase().contains(&query.to_lowercase());
        assert!(
            text_match,
            "Bookmark should match text query: {:?}",
            bookmark
        );
    }

    // Compare with results from just tag filter
    let tag_only_results = service
        .search_bookmarks(
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

    // Should be fewer or equal results when combining filters
    assert!(results.len() <= tag_only_results.len());
}

#[test]
#[serial]
fn given_exact_tag_match_when_search_bookmarks_then_returns_exact_matches_only() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Choose a combination that exists in the test data
    let tags_exact = parse_tags("aaa,bbb");

    let results = service
        .search_bookmarks(
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

    // Check results - each bookmark should have EXACTLY these tags, no more, no less
    for bookmark in &results {
        assert_eq!(bookmark.tags.len(), tags_exact.len());
        assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));
        assert!(bookmark.tags.contains(&Tag::new("bbb").unwrap()));
    }

    // Compare with "all tags" (which allows additional tags)
    let all_tags_results = service
        .search_bookmarks(
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

    // There should be more or equal results with "all tags" than with "exact tags"
    assert!(all_tags_results.len() >= results.len());
}

#[test]
#[serial]
fn given_tag_prefix_when_search_bookmarks_then_returns_matching_prefixed_tags() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Create a prefix tag - this is artificial since we want to test the feature
    // (there may not be explicit prefix matches in the test data)
    let mut prefix_tags = HashSet::new();
    prefix_tags.insert(Tag::new("a").unwrap()); // Should match "aaa"

    let results = service
        .search_bookmarks(
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

    // Check that all results have at least one tag starting with the prefix
    for bookmark in &results {
        let has_prefix_match = bookmark.tags.iter().any(|tag| tag.value().starts_with("a"));
        assert!(
            has_prefix_match,
            "Should have at least one tag starting with 'a': {:?}",
            bookmark
        );
    }
}

#[test]
#[serial]
fn given_negated_tag_filters_when_search_bookmarks_then_excludes_correctly() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Get all bookmarks
    let all_bookmarks = service.get_all_bookmarks(None, None).unwrap();

    // Exclude bookmarks with tag "ccc"
    let tags_any_not = parse_tags("ccc");

    let results = service
        .search_bookmarks(
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

    // Verify no results have the excluded tag
    for bookmark in &results {
        assert!(!bookmark.tags.contains(&Tag::new("ccc").unwrap()));
    }

    // Should be fewer results than total
    assert!(
        results.len() < all_bookmarks.len(),
        "Should have fewer results after exclusion"
    );

    // Count bookmarks with tag "ccc" in original data
    let ccc_count = all_bookmarks
        .iter()
        .filter(|b| b.tags.contains(&Tag::new("ccc").unwrap()))
        .count();

    // Results count should be total minus those with "ccc" tag
    assert_eq!(results.len(), all_bookmarks.len() - ccc_count);
}

#[test]
#[serial]
fn given_sort_direction_and_limit_when_search_bookmarks_then_respects_ordering_and_limits() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Get all bookmarks sorted descending (newest first)
    let desc_results = service
        .search_bookmarks(
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

    // Get all bookmarks sorted ascending (oldest first)
    let _ = service
        .search_bookmarks(
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
    let limited_results = service
        .search_bookmarks(
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
    assert!(limited_results.len() <= limit);

    // Limited results should match the first 'limit' items from unlimited results
    for (limited, unlimited) in limited_results.iter().zip(desc_results.iter()) {
        assert_eq!(limited.id, unlimited.id);
    }
}

#[test]
#[serial]
fn given_highly_specific_filter_combination_when_search_bookmarks_then_filters_apply_in_correct_order(
) {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Create a complex filter:
    // - Text contains "TEST"
    // - Must have tags "aaa" AND "bbb"
    // - Must NOT have tag "xxx"
    // - Limit to top 2 results

    let query = "TEST";
    let tags_all = parse_tags("aaa,bbb");
    let tags_any_not = parse_tags("xxx");
    let limit = 2;

    let results = service
        .search_bookmarks(
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

    // Verify all filters were applied
    for bookmark in &results {
        // Text filter
        let text_match = bookmark
            .title
            .to_lowercase()
            .contains(&query.to_lowercase())
            || bookmark
                .description
                .to_lowercase()
                .contains(&query.to_lowercase())
            || bookmark.url.to_lowercase().contains(&query.to_lowercase());
        assert!(text_match);

        // All tags filter
        assert!(bookmark.tags.contains(&Tag::new("aaa").unwrap()));
        assert!(bookmark.tags.contains(&Tag::new("bbb").unwrap()));

        // Any not filter
        assert!(!bookmark.tags.contains(&Tag::new("xxx").unwrap()));
    }

    // Verify limit
    assert!(results.len() <= limit);

    // Also verify we get the same results with filters applied in different order
    // First apply text filter and tag filters
    let intermediate_results = service
        .search_bookmarks(
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

    // Results should match (have same IDs)
    let result_ids: HashSet<_> = results.iter().filter_map(|b| b.id).collect();
    let manual_ids: HashSet<_> = manually_filtered.iter().filter_map(|b| b.id).collect();

    assert_eq!(
        result_ids, manual_ids,
        "Filters should be applied in a consistent manner"
    );
}

#[test]
#[serial]
fn given_empty_filters_when_search_bookmarks_then_returns_expected_defaults() {
    // Arrange
    let _env = init_test_env();
    let _guard = EnvGuard::new();
    let service = create_test_service();

    // Case 1: All filters None except sort (should return all bookmarks sorted)
    let results_default = service
        .search_bookmarks(
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

    // Should match get_all_bookmarks
    let all_bookmarks = service
        .get_all_bookmarks(Some(SortDirection::Descending), None)
        .unwrap();
    assert_eq!(results_default.len(), all_bookmarks.len());

    // Case 2: Empty text query (should be treated as no text query)
    let results_empty_query = service
        .search_bookmarks(
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

    assert_eq!(results_empty_query.len(), all_bookmarks.len());

    // Case 3: Empty tag sets (should be treated as no tag filter)
    let empty_tags = HashSet::new();
    let results_empty_tags = service
        .search_bookmarks(
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

    assert_eq!(results_empty_tags.len(), all_bookmarks.len());
}
