use bkmr::adapter::dal::Dal;
use bkmr::adapter::embeddings::{Context, DummyAi};
use bkmr::service::embeddings::create_embeddings_for_non_bookmarks;
use bkmr::{helper, CTX};
use camino::Utf8PathBuf;

use rstest::*;
use std::env;


#[cfg(test)]
#[ctor::ctor]
fn init() {
    // CTX.set(Context::new(Box::new(DummyAi))).expect("Failed to set context");
    if CTX.get().is_none() {
        println!("Setting context: DummyAi");
        CTX.set(Context::new(Box::new(DummyAi)))
            .expect("Failed to set context in test initialization");
    }

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
fn test_data_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
}

#[rstest]
fn test_create_embeddings_for_non_bookmarks_non_existing(_dal: Dal, test_data_path: Utf8PathBuf) {
    // Arrange
    // Act
    let result = create_embeddings_for_non_bookmarks(test_data_path);
    // Assert
    assert!(result.is_ok());
    // Add more assertions based on your expectations
}

#[rstest]
fn test_create_embeddings_for_non_bookmarks_existing(_dal: Dal, test_data_path: Utf8PathBuf) {
    // Arrange
    let _ = create_embeddings_for_non_bookmarks(&test_data_path);
    // Act
    let result = create_embeddings_for_non_bookmarks(&test_data_path);
    println!("Result: {:?}", result);
    // Assert
    assert!(result.is_ok());
}
