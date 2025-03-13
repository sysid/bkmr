use bkmr::adapter::dal::{migration, Dal};
use bkmr::model::bms::Bookmarks;
use bkmr::util::testing::init_test_setup;
use rstest::*;
use tracing::debug;

#[ctor::ctor]
fn init() {
    init_test_setup().expect("Failed to initialize test setup");
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    migration::init_db(&mut dal.conn).expect("Error DB init");
}

#[rstest]
fn given_empty_query_when_creating_bookmarks_then_returns_all_bookmarks() {
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
fn given_tag_list_when_checking_unknown_tags_then_returns_expected_count(
    #[case] tags: Vec<String>,
    #[case] expected: usize,
) {
    let mut bms = Bookmarks::new("".to_string());
    let unknown_tags = bms.check_tags(tags).unwrap();
    debug!("{:?}", unknown_tags);
    assert_eq!(unknown_tags.len(), expected);
}

#[rstest]
fn given_tag_set_when_filtering_all_match_then_returns_single_bookmark() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(Some(",xxx,yyy,".to_string()), None, None, None, None);
    assert_eq!(bms.bms.len(), 1);
    assert_eq!(bms.bms[0].id, 2);
}

#[rstest]
fn given_tag_set_when_filtering_all_not_match_then_excludes_matching_bookmark() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, Some(",xxx,yyy,".to_string()), None, None);
    assert_eq!(bms.bms.len(), 10);
    assert_ne!(bms.bms[0].id, 2);
}

#[rstest]
fn given_multiple_tags_when_filtering_any_match_then_returns_matching_bookmarks() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, Some(",xxx,ccc,".to_string()), None, None, None);
    assert_eq!(bms.bms.len(), 4);
}

#[rstest]
fn given_multiple_tags_when_filtering_any_not_match_then_excludes_matching_bookmarks() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, None, Some(",xxx,ccc,".to_string()), None);
    assert_eq!(bms.bms.len(), 7);
}

#[rstest]
fn given_tag_set_when_filtering_exact_match_then_returns_exact_matches() {
    let mut bms = Bookmarks::new("".to_string());
    bms.filter(None, None, None, None, Some(",aaa,bbb,".to_string()));
    assert_eq!(bms.bms.len(), 2);
}
