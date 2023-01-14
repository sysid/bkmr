#![allow(unused_imports, unused_variables)]

use diesel::result::Error as DieselError;
use diesel::sqlite::Sqlite;
use diesel::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::{debug, error, info, log_enabled, Level};
use rstest::*;
use std::collections::HashSet;
use std::error::Error;
use bkmr::dal::Dal;
use bkmr::models::NewBookmark;

mod test_dal;

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}
