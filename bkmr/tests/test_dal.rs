use std::collections::HashSet;

use anyhow::Result;
use bkmr::adapter::dal::Dal;
use bkmr::adapter::embeddings::DummyEmbedding;
use bkmr::context::Context;
use bkmr::model::bookmark::{BookmarkBuilder, BookmarkUpdater};
use bkmr::util::testing::test_dal;
use rstest::rstest;
use tracing::{debug, info};

#[rstest]
fn given_database_when_initializing_then_succeeds(_test_dal: Dal) {
    info!("Initialized DB");
}

#[rstest]
fn given_valid_id_when_getting_bookmark_then_returns_correct_bookmark(
    mut test_dal: Dal,
) -> Result<()> {
    let bm = test_dal.get_bookmark_by_id(1)?;
    println!("The bookmarks are: {:?}", bm);
    assert_eq!(bm.id, 1);
    Ok(())
}

#[rstest]
fn given_invalid_id_when_getting_bookmark_then_returns_error(mut test_dal: Dal) {
    let result = test_dal.get_bookmark_by_id(99999);
    println!("The bookmarks are: {:?}", result);
    assert!(result.is_err());
    // Check error message instead of specific error type since we're using anyhow
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[rstest]
#[case("xxx", 1)]
#[case("", 11)]
#[case("xxxxxxxxxxxxxxxxx", 0)]
fn given_search_query_when_getting_bookmarks_then_returns_matching_results(
    mut test_dal: Dal,
    #[case] input: &str,
    #[case] expected: i32,
) -> Result<()> {
    let bms = test_dal.get_bookmarks(input)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, expected);
    Ok(())
}

#[rstest]
fn given_database_when_getting_bookmarks_without_embedding_then_returns_only_empty_embeddings(
    mut test_dal: Dal,
) -> Result<()> {
    let bookmarks_without_embedding = test_dal.get_bookmarks_without_embedding()?;
    for bookmark in &bookmarks_without_embedding {
        assert!(bookmark.embedding.is_none());
    }
    let expected_count = 11;
    assert_eq!(bookmarks_without_embedding.len(), expected_count);
    Ok(())
}

#[rstest]
#[case("https://www.google.com", true)]
#[case("https://www.doesnotexists.com", false)]
fn given_url_when_checking_existence_then_returns_correct_status(
    mut test_dal: Dal,
    #[case] input: &str,
    #[case] expected: bool,
) -> Result<()> {
    let exists = test_dal.bm_exists(input)?;
    assert_eq!(exists, expected);
    Ok(())
}

#[rstest]
fn given_new_bookmark_when_inserting_then_creates_with_correct_id(mut test_dal: Dal) -> Result<()> {
    Context::update_global(Context::new(Box::new(DummyEmbedding)))?;
    let mut bm = BookmarkBuilder::new()
        .URL("www.sysid.de".to_string())
        .metadata("".to_string())
        .tags(",xxx,".to_string())
        .desc("sysid descript".to_string())
        .flags(0)
        .build();
    bm.update();
    let bms = test_dal.insert_bookmark(bm.convert_to_new_bookmark())?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms[0].id, 12);
    Ok(())
}

#[allow(non_snake_case)]
#[rstest]
fn given_existing_bookmark_when_updating_then_modifies_correctly(mut test_dal: Dal) -> Result<()> {
    let mut bm = test_dal.get_bookmark_by_id(1)?;
    bm.URL = String::from("http://www.sysid.de");
    let bms = test_dal.update_bookmark(bm)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms[0].URL, "http://www.sysid.de");
    Ok(())
}

#[rstest]
fn given_bookmark_when_upserting_then_updates_or_inserts_correctly(
    mut test_dal: Dal,
) -> Result<()> {
    let mut bm = BookmarkBuilder::new()
        .URL("www.sysid.de".to_string())
        .metadata("".to_string())
        .tags(",xxx,".to_string())
        .desc("sysid descript".to_string())
        .flags(0)
        .build();
    bm.update();
    let bms = test_dal.insert_bookmark(bm.convert_to_new_bookmark())?;
    let mut inserted_bm = bms[0].clone();
    assert_eq!(inserted_bm.id, 12);

    inserted_bm.metadata = "xxx".to_string();
    let upserted_bm = test_dal.upsert_bookmark(inserted_bm.convert_to_new_bookmark())?;
    println!("The bookmarks are: {:?}", upserted_bm);
    assert_eq!(upserted_bm[0].id, 12);
    assert_eq!(upserted_bm[0].metadata, "xxx");
    Ok(())
}

#[rstest]
fn given_database_when_cleaning_then_keeps_only_first_entry(mut test_dal: Dal) -> Result<()> {
    test_dal.clean_table()?;
    let bms = test_dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    assert!(ids.contains(&1));
    assert_eq!(ids.len(), 1);
    Ok(())
}

#[rstest]
fn given_bookmark_id_when_batch_executing_then_updates_database_correctly(
    mut test_dal: Dal,
) -> Result<()> {
    test_dal.batch_execute(4)?;
    let bms = test_dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
fn given_bookmark_id_when_deleting_then_removes_and_updates_indices(
    mut test_dal: Dal,
) -> Result<()> {
    let n = test_dal.delete_bookmark2(4)?;
    assert_eq!(n, 1);

    let bms = test_dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
fn given_bookmark_id_when_deleting_directly_then_removes_from_database(
    mut test_dal: Dal,
) -> Result<()> {
    test_dal.delete_bookmark(1)?;
    let bms = test_dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    assert!(!ids.contains(&1));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
#[allow(non_snake_case)]
fn given_database_when_getting_all_tags_then_returns_complete_tag_set(
    mut test_dal: Dal,
) -> Result<()> {
    let tags = test_dal.get_all_tags()?;
    debug!("{:?}", tags);

    let tags_str: Vec<&str> = tags.iter().map(|t| t.tag.as_str()).collect();
    println!("The bookmarks are: {:?}", tags_str);

    let expected: HashSet<&str> = ["ccc", "bbb", "aaa", "yyy", "xxx"]
        .iter()
        .cloned()
        .collect();
    let result: HashSet<&str> = tags_str.iter().cloned().collect();
    assert_eq!(result, expected);
    Ok(())
}

#[rstest]
fn given_database_when_getting_all_tags_as_vector_then_returns_sorted_list(
    mut test_dal: Dal,
) -> Result<()> {
    let tags = test_dal.get_all_tags_as_vec()?;
    debug!("{:?}", tags);
    assert_eq!(tags.len(), 5);
    assert_eq!(tags, vec!["aaa", "bbb", "ccc", "xxx", "yyy"]);
    Ok(())
}

#[rstest]
fn given_tag_when_getting_related_tags_then_returns_associated_tags(
    mut test_dal: Dal,
) -> Result<()> {
    let tags = test_dal.get_related_tags("ccc")?;
    let tags_str: Vec<&str> = tags.iter().map(|t| t.tag.as_str()).collect();

    let expected: HashSet<&str> = ["ccc", "bbb", "aaa", "yyy", "xxx"]
        .iter()
        .cloned()
        .collect();
    let result: HashSet<&str> = tags_str.iter().cloned().collect();
    assert_eq!(result, expected);
    Ok(())
}

#[rstest]
fn given_count_when_getting_random_bookmarks_then_returns_requested_number(
    mut test_dal: Dal,
) -> Result<()> {
    let bms = test_dal.get_randomized_bookmarks(2)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, 2);
    Ok(())
}

#[rstest]
fn given_count_when_getting_oldest_bookmarks_then_returns_oldest_entries(
    mut test_dal: Dal,
) -> Result<()> {
    let bms = test_dal.get_oldest_bookmarks(2)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, 2);
    Ok(())
}

#[rstest]
fn given_database_when_checking_schema_migrations_then_confirms_existence(
    mut test_dal: Dal,
) -> Result<()> {
    let exists = test_dal.check_schema_migrations_exists()?;
    println!("Result: {:?}", exists);
    assert!(exists);
    Ok(())
}

#[rstest]
fn given_database_when_checking_embedding_column_then_confirms_existence(
    mut test_dal: Dal,
) -> Result<()> {
    let exists = test_dal.check_embedding_column_exists()?;
    println!("Result: {:?}", exists);
    assert!(exists);
    Ok(())
}
