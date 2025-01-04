use anyhow::Result;
use rstest::*;

use bkmr::adapter::dal::Dal;
use bkmr::context::CTX;
use bkmr::util::testing::init_test_setup;
use bkmr::util::testing::test_dal;
use bkmr::{load_url_details, update_bm, update_bookmarks};

#[ctor::ctor]
fn init() {
    init_test_setup().expect("Failed to initialize test setup");
}

#[rstest]
// #[ignore = "seems to hang in Pycharm, but not Makefile"]
fn given_valid_url_when_loading_details_then_returns_correct_metadata() {
    let result = load_url_details("https://www.rust-lang.org/");
    println!("Result: {:?}", result);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().0, "Rust Programming Language");
}

#[rstest]
#[case(1, vec![], vec![], false, ",ccc,yyy,".to_string())]
#[case(1, vec!["t1".to_string(), "t2".to_string()], vec![], false, ",ccc,t1,t2,yyy,".to_string())]
#[case(1, vec!["t1".to_string(), "t2".to_string()], vec![], true, ",t1,t2,".to_string())]
#[case(1, vec![], vec!["ccc".to_string()], false, ",yyy,".to_string())]
fn given_bookmark_id_when_updating_with_tags_then_modifies_correctly(
    mut test_dal: Dal,
    #[case] id: i32,
    #[case] tags: Vec<String>,
    #[case] tags_not: Vec<String>,
    #[case] force: bool,
    #[case] expected: String,
) -> Result<()> {
    update_bm(id, &tags, &tags_not, &mut test_dal, force)?;

    let bm = test_dal.get_bookmark_by_id(id)?;
    assert_eq!(bm.tags, expected);
    println!("bm: {:?}", bm);
    Ok(())
}

#[rstest]
fn given_bookmark_when_updating_then_succeeds(mut test_dal: Dal) -> Result<()> {
    update_bm(1, &vec![], &vec![], &mut test_dal, false)?;
    Ok(())
}

#[rstest]
fn given_bookmark_list_when_updating_multiple_then_succeeds() {
    let (ids, tags, tags_not, force) = (
        vec![1],
        vec!["t1".to_string(), "t2".to_string()],
        vec![],
        false,
    );
    let result = update_bookmarks(ids, tags, tags_not, force);
    assert!(result.is_ok());
}

// #[rstest]
// fn test_add_bm(mut dal: Dal) {
//     let bm = NewBookmark {
//         URL: "https://www.rust-lang.org/".to_string(),
//         metadata: "The Rust Programming Language".to_string(),
//         tags: ",ccc,yyy,".to_string(),
//         ..Default::default()
//     };
//     let _ = add_bm();
// }

#[rstest]
fn given_context_when_initializing_then_exists() {
    assert!(CTX.get().is_some());
}
