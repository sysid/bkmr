use log::debug;
use rstest::*;

use bkmr::adapter::dal::Dal;
use bkmr::helper;
use bkmr::model::bms::Bookmarks;

#[ctor::ctor]
fn init() {
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    helper::init_db(&mut dal.conn).expect("Error DB init");
}

#[rstest]
fn test_init_bms() {
    let bms = Bookmarks::new("".to_string());
    assert_eq!(bms.bms.len(), 11);
}

// #[rstest]
// fn test_bms_embed() {
//     let mut bms = Bookmarks::new("".to_string());
//     bms.embed();
//     assert_eq!(bms.bms.len(), 11);
// }

#[rstest]
#[case(vec ! [String::from("aaa"), String::from("bbb")], 0)]
#[case(vec ! [String::from("xyz")], 1)]
#[case(vec ! [String::from("")], 0)]
#[case(vec ! [], 0)]
fn test_check_tags(#[case] tags: Vec<String>, #[case] expected: usize) {
    let mut bms = Bookmarks::new("".to_string());
    let unknown_tags = bms.check_tags(tags).unwrap();
    debug!("{:?}", unknown_tags);
    assert_eq!(unknown_tags.len(), expected);
}

#[rstest]
fn test_match_all() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(Some(",xxx,yyy,".to_string()), None, None, None, None);
    assert_eq!(bms.bms.len(), 1);
    assert_eq!(bms.bms[0].id, 2);
}

#[rstest]
fn test_match_all_not() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, Some(",xxx,yyy,".to_string()), None, None);
    assert_eq!(bms.bms.len(), 10);
    assert_ne!(bms.bms[0].id, 2);
}

#[rstest]
fn test_match_any() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, Some(",xxx,ccc,".to_string()), None, None, None);
    assert_eq!(bms.bms.len(), 4);
}

#[rstest]
fn test_match_any_not() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, None, Some(",xxx,ccc,".to_string()), None);
    assert_eq!(bms.bms.len(), 7);
}

#[rstest]
fn test_match_exact() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, None, None, Some(",aaa,bbb,".to_string()));
    assert_eq!(bms.bms.len(), 2);
}
