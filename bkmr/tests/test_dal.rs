use std::collections::HashSet;


use anyhow::Result;
use log::{debug, info};
use rstest::{fixture, rstest};
use stdext::function_name;

use bkmr::adapter::dal::Dal;
use bkmr::adapter::embeddings::{Context, DummyAi};
use bkmr::model::bookmark::{BookmarkBuilder, BookmarkUpdater};
use bkmr::{helper, CTX};

#[fixture]
pub fn dal() -> Dal {
    helper::init_logger();
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    helper::init_db(&mut dal.conn).expect("Error DB init");
    dal
}

#[rstest]
fn test_init_db(mut dal: Dal) {
    helper::init_db(&mut dal.conn).expect("Error DB init");
    info!("Init DB");
    assert!(true);
}

#[rstest]
#[ignore = "!!!DANGER!!!"]
fn test_danger() {
    helper::init_logger();
    let mut dal = Dal::new(String::from(
        "/Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737",
    ));
    let bm = dal.get_bookmark_by_id(1111);
    println!("The bookmarks are: {:?}", bm);
    // assert_eq!(bm.unwrap().id, 1);
}

#[rstest]
fn test_get_bookmark_by_id(mut dal: Dal) {
    let bm = dal.get_bookmark_by_id(1);
    println!("The bookmarks are: {:?}", bm);
    assert_eq!(bm.unwrap().id, 1);
}

#[rstest]
// #[should_panic(expected = "NotFound")]
fn test_get_bookmark_by_id_non_existing(mut dal: Dal) {
    let bm = dal.get_bookmark_by_id(99999);
    println!("The bookmarks are: {:?}", bm);
    // assert_eq!(bm.unwrap().id, 1);
    assert!(bm.is_err());
    assert!(matches!(bm, Err(diesel::result::Error::NotFound)));
}

#[rstest]
#[case("xxx", 1)]
#[case("", 11)]
#[case("xxxxxxxxxxxxxxxxx", 0)]
fn test_get_bookmarks(mut dal: Dal, #[case] input: &str, #[case] expected: i32) {
    let bms = dal.get_bookmarks(input);
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.unwrap().len() as i32, expected);
}

#[rstest]
fn test_get_bookmarks_without_embedding(mut dal: Dal) {
    let bookmarks_without_embedding = dal.get_bookmarks_without_embedding().unwrap();
    for bookmark in &bookmarks_without_embedding {
        assert!(bookmark.embedding.is_none());
    }
    let expected_count = 11;
    assert_eq!(bookmarks_without_embedding.len(), expected_count);
}

#[rstest]
#[case("https://www.google.com", true)]
#[case("https://www.doesnotexists.com", false)]
fn test_bm_exists(mut dal: Dal, #[case] input: &str, #[case] expected: bool) {
    let exists = dal.bm_exists(input);
    // println!("The bookmarks are: {:?}", bms);
    assert_eq!(exists.unwrap(), expected);
}

#[rstest]
fn test_insert_bm(mut dal: Dal) {
    // init_db(&mut dal.conn).expect("Error DB init");
    if CTX.get().is_none() {
        CTX.set(Context::new(Box::new(DummyAi))).unwrap();
    }
    let mut bm = BookmarkBuilder::new()
        .URL("www.sysid.de".to_string())
        .metadata("".to_string())
        .tags(",xxx,".to_string())
        .desc("sysid descript".to_string())
        .flags(0)
        .build();
    bm.update();
    let bms = dal.insert_bookmark(bm.convert_to_new_bookmark());
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.unwrap()[0].id, 12);
}

#[allow(non_snake_case)]
#[rstest]
fn test_update_bm(mut dal: Dal) {
    let mut bm = dal.get_bookmark_by_id(1).unwrap();
    // init_db(&mut dal.conn).expect("Error DB init");
    bm.URL = String::from("http://www.sysid.de");
    let bms = dal.update_bookmark(bm);
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.unwrap()[0].URL, "http://www.sysid.de");
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
fn test_clean_table(mut dal: Dal) {
    let _bms = dal.clean_table();
    let mut ids = Vec::new();
    let bms = dal.get_bookmarks("").unwrap();
    for (i, _bm) in bms.iter().enumerate() {
        ids.push(bms[i].id)
    }
    // println!("The ids are: {:?}", ids);
    assert!(ids.contains(&1));
    assert_eq!(ids.len(), 1);
}

#[rstest]
fn test_batch_execute(mut dal: Dal) {
    dal.batch_execute(4).unwrap(); // asdf2
    let mut ids = Vec::new();

    let bms = dal.get_bookmarks("").unwrap();
    for (i, _bm) in bms.iter().enumerate() {
        ids.push(bms[i].id)
    }
    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
}

#[rstest]
fn test_delete_bm2(mut dal: Dal) {
    let n = dal.delete_bookmark2(4).unwrap(); // asdf2
    let mut ids = Vec::new();
    assert_eq!(n, 1);

    let bms = dal.get_bookmarks("").unwrap();
    for (i, _bm) in bms.iter().enumerate() {
        ids.push(bms[i].id)
    }
    println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&11));
    assert_eq!(ids.len(), 10);
}

#[rstest]
fn test_delete_bm(mut dal: Dal) {
    let _bms = dal.delete_bookmark(1);
    let mut ids = Vec::new();
    let bms = dal.get_bookmarks("").unwrap();
    for (i, _bm) in bms.iter().enumerate() {
        ids.push(bms[i].id)
    }
    // println!("The ids are: {:?}", ids);
    assert!(!ids.contains(&1));
    assert_eq!(ids.len(), 10);
}

#[rstest]
#[allow(non_snake_case)]
fn test__get_all_tags(mut dal: Dal) {
    let tags = dal.get_all_tags().unwrap();
    debug!("({}:{}) {:?}", function_name!(), line!(), tags);

    let mut tags_str: Vec<&str> = Vec::new();
    for (i, _t) in tags.iter().enumerate() {
        tags_str.push(&tags[i].tag);
    }
    println!("The bookmarks are: {:?}", tags_str);
    let expected: HashSet<&str> = ["ccc", "bbb", "aaa", "yyy", "xxx"]
        .iter()
        .cloned()
        .collect();
    let result: HashSet<&str> = tags_str.iter().cloned().collect();
    assert_eq!(result, expected);
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
fn test_get_related_tags(mut dal: Dal) {
    let tags = dal.get_related_tags("ccc").unwrap();
    let mut tags_str: Vec<&str> = Vec::new();
    for (i, _t) in tags.iter().enumerate() {
        tags_str.push(&tags[i].tag);
    }
    let expected: HashSet<&str> = ["ccc", "bbb", "aaa", "yyy", "xxx"]
        .iter()
        .cloned()
        .collect();
    let result: HashSet<&str> = tags_str.iter().cloned().collect();
    assert_eq!(result, expected);
}

#[rstest]
fn test_get_randomized_bookmarks(mut dal: Dal) {
    let bms = dal.get_randomized_bookmarks(2);
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.unwrap().len() as i32, 2);
}

#[rstest]
fn test_get_oldest_bookmarks(mut dal: Dal) {
    let bms = dal.get_oldest_bookmarks(2);
    println!("The bookmarks are: {:?}", bms);
    assert_eq!(bms.unwrap().len() as i32, 2);
}

#[rstest]
fn test_check_schema_migration_exists(mut dal: Dal) {
    let result = dal.check_schema_migrations_exists();
    println!("Result: {:?}", result);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[rstest]
fn test_check_embedding_column_exists(mut dal: Dal) {
    let result = dal.check_embedding_column_exists();
    println!("Result: {:?}", result);
    assert!(result.is_ok());
    assert!(result.unwrap());
}
