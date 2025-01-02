use anyhow::Result;
use rstest::*;

use bkmr::adapter::dal::Dal;
use bkmr::model::bookmark::Bookmark;
use bkmr::util::testing::{init_test_setup, setup_test_db};
use bkmr::{load_url_details, update_bm, update_bookmarks};
use bkmr::context::CTX;

mod test_dal;

mod adapter {
    mod test_json;
}

mod service {
    mod test_embeddings;
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    init_test_setup().expect("Failed to initialize test setup");
}

#[fixture]
pub fn dal() -> Dal {
    setup_test_db().expect("Failed to set up test database")
}

#[fixture]
fn bms() -> Vec<Bookmark> {
    let mut dal = setup_test_db().expect("Failed to set up test database");
    dal.get_bookmarks("").expect("Failed to get bookmarks")
}

#[rstest]
// #[ignore = "seems to hang in Pycharm, but not Makefile"]
fn test_load_url_details() {
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
fn test_update_bm(
    mut dal: Dal,
    #[case] id: i32,
    #[case] tags: Vec<String>,
    #[case] tags_not: Vec<String>,
    #[case] force: bool,
    #[case] expected: String,
) -> Result<()> {
    update_bm(id, &tags, &tags_not, &mut dal, force)?;

    let bm = dal.get_bookmark_by_id(id)?;
    assert_eq!(bm.tags, expected);
    println!("bm: {:?}", bm);
    Ok(())
}

#[rstest]
fn test_upd(mut dal: Dal) -> Result<()> {
    update_bm(1, &vec![], &vec![], &mut dal, false)?;
    Ok(())
}

#[rstest]
fn test_update_bookmarks_successful() {
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
fn test_ctx() {
    assert!(CTX.get().is_some());
}
