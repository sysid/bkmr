use bkmr::dal::Dal;
use bkmr::helper;
use bkmr::models::Bookmark;
use bkmr::process::{delete_bms, do_edit};
use rstest::{fixture, rstest};

#[fixture]
pub fn dal() -> Dal {
    helper::init_logger();
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    helper::init_db(&mut dal.conn).expect("Error DB init");
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
