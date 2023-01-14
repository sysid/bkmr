use rstest::{fixture, rstest};
use twbm::dal::Dal;
use twbm::helper;
use twbm::models::Bookmark;
use twbm::process::{delete_bms, do_edit};

#[fixture]
pub fn dal() -> Dal {
    helper::init_logger();
    let mut dal = Dal::new(String::from("../db/twbm.db"));
    helper::init_db(&mut dal.conn).expect("Error DB init");
    dal
}

#[fixture]
fn bms() -> Vec<Bookmark> {
    let mut dal = Dal::new(String::from("../db/twbm.db"));
    // init_db(&mut dal.conn).expect("Error DB init");
    let bms = dal.get_bookmarks("");
    bms.unwrap()
}

#[rstest]
#[ignore = "Manual Test: make test-vim"]
fn test_do_edit(mut dal: Dal, bms: Vec<Bookmark>) {
    let bm = bms[0].clone();
    do_edit(&bm).unwrap();

    println!("{:#?}", dal.get_bookmark_by_id(bm.id).unwrap());
}

// #[allow(unused_variables)]
#[rstest]
fn test_delete_bms(mut dal: Dal, bms: Vec<Bookmark>) {
    let _ = bms[0].clone();
    // make sure input is sorted as it would be using the helper
    let ids = helper::ensure_int_vector(&vec!["6".to_string(), "2".to_string(), "3".to_string()]);
    delete_bms(ids.unwrap(), bms).unwrap();

    assert_eq!(dal.get_bookmarks("").unwrap().len(), 8);
    assert_eq!(dal.get_bookmarks("bbbbb").unwrap().len(), 0);
    assert_eq!(dal.get_bookmarks("yyyyy").unwrap().len(), 0);
    assert_eq!(dal.get_bookmarks("11111").unwrap().len(), 0);
}
