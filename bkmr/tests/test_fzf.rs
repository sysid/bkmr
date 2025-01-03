// #![allow(unused_imports, unused_variables)]
use rstest::*;

use bkmr::adapter::dal::{migration, Dal};
use bkmr::model::bookmark::Bookmark;
use bkmr::service::fzf::fzf_process;

mod test_dal;

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

/// uses interactive console
#[rstest]
#[ignore = "Interactive via Makefile"]
fn test_fzf(bms: Vec<Bookmark>) {
    fzf_process(&bms);
}
