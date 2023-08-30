use log::{debug, info};
use rstest::{fixture, rstest};
use std::collections::HashSet;
use stdext::function_name;
// use stdext::function_name;
use bkmr::dal::Dal;
use bkmr::helper;
use bkmr::models::NewBookmark;

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
#[should_panic(expected = "NotFound")]
fn test_get_bookmark_by_id_non_existing(mut dal: Dal) {
    let bm = dal.get_bookmark_by_id(99999);
    println!("The bookmarks are: {:?}", bm);
    assert_eq!(bm.unwrap().id, 1);
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
    #[allow(non_snake_case)]
    let new_bm = NewBookmark {
        URL: String::from("http://www.sysid.de"),
        metadata: String::from(""),
        tags: String::from(",xxx,"),
        desc: String::from("sysid descript"),
        flags: 0,
    };
    let bms = dal.insert_bookmark(new_bm);
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
fn test_get_all_tags(mut dal: Dal) {
    let tags = dal.get_all_tags_as_vec();
    debug!("{:?}", tags);
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

