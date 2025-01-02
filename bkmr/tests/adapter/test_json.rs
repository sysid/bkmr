use camino::Utf8PathBuf;
use rstest::*;
use std::env;
use tracing::debug;
use bkmr::adapter::json::read_ndjson_file_and_create_bookmarks;


#[fixture]
fn test_data_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
}

#[rstest]
fn test_read_ndjson_file_and_create_bookmarks(test_data_path: Utf8PathBuf) {
    // CTX.set(Context::new(Box::new(bkmr::embeddings::DummyAi::default()))).unwrap();
    debug!("Path: {:?}", test_data_path);
    let bookmarks =
        read_ndjson_file_and_create_bookmarks(test_data_path).expect("Failed to read .ndjson file");
    debug!("Bookmarks: {:?}", bookmarks);

    assert_eq!(bookmarks.len(), 3);
    // Update the assertions to check the properties of the Bookmark instances
    // Replace 'id' and 'content' with the actual properties of the Bookmark struct
    assert_eq!(bookmarks[0].URL, "/a/b/readme.md:0");
    assert_eq!(bookmarks[0].metadata, "readme.md");
    assert_eq!(bookmarks[0].tags, ",_imported_,");
    assert_eq!(bookmarks[0].desc, "First record");

    assert_eq!(bookmarks[1].URL, "/a/b/readme.md:1");
    assert_eq!(bookmarks[2].URL, "/a/b/c/xxx.md:0");
}
