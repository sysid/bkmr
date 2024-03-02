use std::env;
use log::debug;
use rstest::*;
use camino::Utf8PathBuf;

use bkmr::adapter::json::read_ndjson_file;

#[fixture]
fn test_data_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
}

#[rstest]
fn test_read_ndjson_file(test_data_path: Utf8PathBuf) {
    debug!("Path: {:?}", test_data_path);
    let records = read_ndjson_file(test_data_path).expect("Failed to read .ndjson file");
    debug!("Records: {:?}", records);

    assert_eq!(records.len(), 3);
    assert_eq!(records[0].id, "/a/b/readme.md:0");
    assert_eq!(records[0].content, "First record");
    assert_eq!(records[1].id, "/a/b/readme.md:1");
    assert_eq!(records[1].content, "Second record");
    assert_eq!(records[2].id, "/a/b/c/xxx.md:0");
    assert_eq!(records[2].content, "Third record");
}