use std::collections::HashSet;


use anyhow::Result;
use rstest::{fixture, rstest};
use tracing::{debug, info};
use bkmr::adapter::dal::{migration, Dal};
use bkmr::adapter::embeddings::DummyEmbedding;
use bkmr::context::Context;
use bkmr::model::bookmark::{BookmarkBuilder, BookmarkUpdater};

#[fixture]
pub fn dal() -> Dal {
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    migration::init_db(&mut dal.conn).expect("Error DB init");
    dal
}

#[rstest]
fn test_init_db(_dal: Dal) {
    info!("Init DB");
}

#[rstest]
fn test_get_bookmark_by_id(mut dal: Dal) -> Result<()> {
    let bm = dal.get_bookmark_by_id(1)?;
    println!("The bookmarks are: {:?}", bm);
    assert_eq!(bm.id, 1);
    Ok(())
}

#[rstest]
fn test_get_bookmark_by_id_non_existing(mut dal: Dal) {
    let result = dal.get_bookmark_by_id(99999);
    println!("The bookmarks are: {:?}", result);
    assert!(result.is_err());
    // Check error message instead of specific error type since we're using anyhow
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[rstest]
#[case("xxx", 1)]
#[case("", 11)]
#[case("xxxxxxxxxxxxxxxxx", 0)]
fn test_get_bookmarks(mut dal: Dal, #[case] input: &str, #[case] expected: i32) -> Result<()> {
    let bms = dal.get_bookmarks(input)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, expected);
    Ok(())
}

#[rstest]
fn test_get_bookmarks_without_embedding(mut dal: Dal) -> Result<()> {
    let bookmarks_without_embedding = dal.get_bookmarks_without_embedding()?;
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
fn test_bm_exists(mut dal: Dal, #[case] input: &str, #[case] expected: bool) -> Result<()> {
    let exists = dal.bm_exists(input)?;
    assert_eq!(exists, expected);
    Ok(())
}

#[rstest]
fn test_insert_bm(mut dal: Dal) -> Result<()> {
    Context::update_global(Context::new(Box::new(DummyEmbedding)))?;
    let mut bm = BookmarkBuilder::new()
        .URL("www.sysid.de".to_string())
        .metadata("".to_string())
        .tags(",xxx,".to_string())
        .desc("sysid descript".to_string())
        .flags(0)
        .build();
    bm.update();
    let bms = dal.insert_bookmark(bm.convert_to_new_bookmark())?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms[0].id, 12);
    Ok(())
}

#[allow(non_snake_case)]
#[rstest]
fn test_update_bm(mut dal: Dal) -> Result<()> {
    let mut bm = dal.get_bookmark_by_id(1)?;
    bm.URL = String::from("http://www.sysid.de");
    let bms = dal.update_bookmark(bm)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms[0].URL, "http://www.sysid.de");
    Ok(())
}

#[rstest]
fn test_upsert_bookmark(mut dal: Dal) -> Result<()> {
    let mut bm = BookmarkBuilder::new()
        .URL("www.sysid.de".to_string())
        .metadata("".to_string())
        .tags(",xxx,".to_string())
        .desc("sysid descript".to_string())
        .flags(0)
        .build();
    bm.update();
    let bms = dal.insert_bookmark(bm.convert_to_new_bookmark())?;
    let mut inserted_bm = bms[0].clone();
    assert_eq!(inserted_bm.id, 12);

    inserted_bm.metadata = "xxx".to_string();
    let upserted_bm = dal.upsert_bookmark(inserted_bm.convert_to_new_bookmark())?;
    println!("The bookmarks are: {:?}", upserted_bm);
    assert_eq!(upserted_bm[0].id, 12);
    assert_eq!(upserted_bm[0].metadata, "xxx");
    Ok(())
}

#[rstest]
fn test_clean_table(mut dal: Dal) -> Result<()> {
    dal.clean_table()?;
    let bms = dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    assert!(ids.contains(&1));
    assert_eq!(ids.len(), 1);
    Ok(())
}

#[rstest]
fn test_batch_execute(mut dal: Dal) -> Result<()> {
    dal.batch_execute(4)?;
    let bms = dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
fn test_delete_bm2(mut dal: Dal) -> Result<()> {
    let n = dal.delete_bookmark2(4)?;
    assert_eq!(n, 1);

    let bms = dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
fn test_delete_bm(mut dal: Dal) -> Result<()> {
    dal.delete_bookmark(1)?;
    let bms = dal.get_bookmarks("")?;
    let ids: Vec<i32> = bms.iter().map(|bm| bm.id).collect();

    assert!(!ids.contains(&1));
    assert_eq!(ids.len(), 10);
    Ok(())
}

#[rstest]
#[allow(non_snake_case)]
fn test__get_all_tags(mut dal: Dal) -> Result<()> {
    let tags = dal.get_all_tags()?;
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
fn test_get_all_tags(mut dal: Dal) -> Result<()> {
    let tags = dal.get_all_tags_as_vec()?;
    debug!("{:?}", tags);
    assert_eq!(tags.len(), 5);
    assert_eq!(tags, vec!["aaa", "bbb", "ccc", "xxx", "yyy"]);
    Ok(())
}

#[rstest]
fn test_get_related_tags(mut dal: Dal) -> Result<()> {
    let tags = dal.get_related_tags("ccc")?;
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
fn test_get_randomized_bookmarks(mut dal: Dal) -> Result<()> {
    let bms = dal.get_randomized_bookmarks(2)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, 2);
    Ok(())
}

#[rstest]
fn test_get_oldest_bookmarks(mut dal: Dal) -> Result<()> {
    let bms = dal.get_oldest_bookmarks(2)?;
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.len() as i32, 2);
    Ok(())
}

#[rstest]
fn test_check_schema_migration_exists(mut dal: Dal) -> Result<()> {
    let exists = dal.check_schema_migrations_exists()?;
    println!("Result: {:?}", exists);
    assert!(exists);
    Ok(())
}

#[rstest]
fn test_check_embedding_column_exists(mut dal: Dal) -> Result<()> {
    let exists = dal.check_embedding_column_exists()?;
    println!("Result: {:?}", exists);
    assert!(exists);
    Ok(())
}