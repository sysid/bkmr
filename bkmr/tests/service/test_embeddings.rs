use camino::Utf8PathBuf;
use rstest::*;
use std::env;

use bkmr::adapter::{
    dal::Dal
    ,  // Updated imports
};
use bkmr::adapter::dal::migration;
use bkmr::context::CTX;
use bkmr::service::embeddings::create_embeddings_for_non_bookmarks;

#[fixture]
pub fn dal() -> Dal {
    let mut dal = Dal::new(String::from("../db/bkmr.db"));
    migration::init_db(&mut dal.conn).expect("Error DB init");
    dal
}

#[fixture]
fn test_data_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
}

#[rstest]
fn test_create_embeddings_for_non_bookmarks_non_existing(dal: Dal, test_data_path: Utf8PathBuf) {
    // Arrange
    let _ = dal; // Explicitly show we need the DAL fixture

    // Act
    let result = create_embeddings_for_non_bookmarks(&test_data_path);

    // Assert
    assert!(result.is_ok(), "Failed to create embeddings: {:?}", result);
}

#[rstest]
fn test_create_embeddings_for_non_bookmarks_existing(dal: Dal, test_data_path: Utf8PathBuf) {
    // Arrange
    let _ = dal; // Explicitly show we need the DAL fixture
    let first_run = create_embeddings_for_non_bookmarks(&test_data_path);
    assert!(first_run.is_ok(), "First run failed: {:?}", first_run);

    // Act
    let result = create_embeddings_for_non_bookmarks(&test_data_path);

    // Assert
    assert!(result.is_ok(), "Second run failed: {:?}", result);
}

// Add helper test to verify context is properly set
#[test]
fn test_context_initialization() {
    assert!(CTX.get().is_some(), "Global context should be initialized");
}