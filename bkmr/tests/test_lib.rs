#![allow(unused_imports, unused_variables)]

use bkmr::dal::Dal;
use diesel::result::Error as DieselError;
use diesel::sqlite::Sqlite;
use diesel::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, log_enabled, Level};
use rstest::*;
use std::collections::HashSet;
use std::env;
use std::error::Error;
// use bkmr::fzf;
use bkmr::models::{Bookmark, NewBookmark};
use bkmr::{helper, load_url_details};
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

#[rstest]
fn test_load_url_details() {
    let result = load_url_details("https://www.rust-lang.org/");
    assert!(result.is_ok());
    // assert_eq!(result.unwrap().title, "The Rust Programming Language");
    // println!("Result: {:?}", result);
}
