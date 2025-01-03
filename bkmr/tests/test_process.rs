use bkmr::adapter::dal::{migration, Dal};
use bkmr::model::bookmark::Bookmark;
use bkmr::service::process::{delete_bms, do_edit, do_touch};
use bkmr::util::helper;
use rstest::{fixture, rstest};
use std::thread::sleep;
use std::time::Duration;

#[fixture]
pub fn dal() -> Dal {
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    migration::init_db(&mut dal.conn).expect("Error DB init");
    dal
}

#[fixture]
fn bms() -> Vec<Bookmark> {
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    // init_db(&mut dal.conn).expect("Error DB init");
    let bms = dal.get_bookmarks("");
    bms.unwrap()
}
#[rstest]
fn test_do_touch(mut dal: Dal) -> anyhow::Result<()> {

    let bm_before = dal.get_bookmark_by_id(1)?;
    sleep(Duration::from_secs(1));
    do_touch(&bm_before)?;
    let bm_after = dal.get_bookmark_by_id(1)?;
    assert!(bm_before.last_update_ts < bm_after.last_update_ts);
    assert_eq!(bm_before.tags, bm_after.tags);
    assert_eq!(bm_before.flags + 1, bm_after.flags);
    Ok(())
}

#[rstest]
#[ignore = "Manual Test: make test-vim"]
fn test_do_edit(mut dal: Dal, bms: Vec<Bookmark>) {
    let bm = bms[0].clone();
    // avoid panic as it would with CLI call
    do_edit(&bm).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    println!("{:#?}", dal.get_bookmark_by_id(bm.id).unwrap());
}

// #[allow(unused_variables)]
#[rstest]
fn test_delete_bms(mut dal: Dal, bms: Vec<Bookmark>) {
    let _ = bms[0].clone();
    // make sure input is sorted as it would be using the helper
    let ids = helper::ensure_int_vector(&vec!["6".to_string(), "2".to_string(), "3".to_string()]);
    // let ids = helper::ensure_int_vector(&vec!["6".to_string()]);
    delete_bms(ids.unwrap(), bms).unwrap();

    assert_eq!(dal.get_bookmarks("").unwrap().len(), 8);
    assert_eq!(dal.get_bookmarks("bbbbb").unwrap().len(), 0);
    assert_eq!(dal.get_bookmarks("yyyyy").unwrap().len(), 0);
    assert_eq!(dal.get_bookmarks("11111").unwrap().len(), 0);
}
