#![allow(unused_imports, unused_variables)]

use bkmr::dal::Dal;
use bkmr::fzf::fzf_process;
use bkmr::helper;
use bkmr::models::{Bookmark, NewBookmark};
use diesel::result::Error as DieselError;
use diesel::sqlite::Sqlite;
use diesel::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, log_enabled, Level};
use rstest::*;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use stdext::function_name;

mod test_dal;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env::set_var("SKIM_LOG", "info");
    env::set_var("TUIKIT_LOG", "info");
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        .filter_module("skim", log::LevelFilter::Info)
        .filter_module("tuikit", log::LevelFilter::Info)
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}

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

/// uses interactive console
#[rstest]
#[ignore = "Interactive via Makefile"]
fn test_fzf(bms: Vec<Bookmark>) {
    let ids = fzf_process(&bms);
}
