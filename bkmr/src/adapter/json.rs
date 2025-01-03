#![allow(non_snake_case)]

use crate::util::helper::extract_filename;
use crate::model::bookmark::{Bookmark, BookmarkBuilder};
use anyhow::Context;
use camino::Utf8Path;
use chrono::NaiveDateTime;
use serde_derive::Serialize;
use std::fs::File;
use std::io;
use std::io::{BufRead, Write};

/// Checks the format of a JSON string.
///
/// This function takes a JSON string as input and checks if it conforms to the expected format.
/// The expected format is: {"id": "/a/b/readme.md:0", "content": "First record"}
///
/// # Arguments
///
/// * `line` - A string slice that holds the JSON string.
///
/// # Returns
///
/// * `anyhow::Result<()>` - Returns `Ok(())` if the JSON string conforms to the expected format,
/// otherwise returns an `Err` with a message indicating the invalid JSON format.
///
/// # Errors
///
/// This function will return an error if the JSON string does not conform to the expected format.
pub fn check_json_format(line: &str) -> anyhow::Result<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .with_context(|| format!("Failed to deserialize line: {}", line))?;
    if record["id"].is_null() || record["content"].is_null() {
        return Err(anyhow::anyhow!("Invalid JSON format"));
    }
    Ok(())
}

/// Reads a newline-delimited JSON (NDJSON) file and creates bookmarks from each line.
/// format: {"id": "/a/b/readme.md:0", "content": "First record"}
///
/// Mappings:
/// - `id` -> `URL`
/// - `id` -> `metadata` (filename)
/// - `content` -> `desc`
pub fn read_ndjson_file_and_create_bookmarks<P>(file_path: P) -> anyhow::Result<Vec<Bookmark>>
where
    P: AsRef<Utf8Path> + std::fmt::Display,
{
    let file = File::open(file_path.as_ref())
        .with_context(|| format!("Failed to open file {:?}", file_path.as_ref()))?;
    let reader = io::BufReader::new(file);
    let mut bookmarks = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| "Failed to read line from file")?;
        check_json_format(&line).with_context(|| format!("Reading: {}", file_path))?;
        let record: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to deserialize line: {}", line))?;

        // todo: ensure exists and is uniq
        let id = record["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid ID format"))?
            .to_string();
        let filename = extract_filename(&id);
        let bookmark = BookmarkBuilder::new()
            .id(1)
            .URL(id) // Using the content field as the URL for illustration
            .metadata(filename) // Add this if you have the data
            .desc(record["content"].as_str().unwrap_or_default().to_string()) // Add this if you have the data
            .tags(",_imported_,".to_string()) // Add this if you have the data
            .build();
        bookmarks.push(bookmark);
    }
    Ok(bookmarks)
}

pub fn bms_to_json(bms: &Vec<Bookmark>) {
    let bms_view: Vec<BookmarkView> = bms.iter().map(BookmarkView::from).collect();
    let json =
        serde_json::to_string_pretty(&bms_view).expect("Failed to serialize bookmarks to JSON.");
    io::stdout()
        .write_all(json.as_bytes())
        .expect("Failed to write JSON to stdout.");
    println!();
}

#[derive(Serialize)]
pub struct BookmarkView {
    pub id: i32,
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    #[serde(with = "serde_with::chrono::NaiveDateTime")]
    pub last_update_ts: NaiveDateTime,
}

impl From<&Bookmark> for BookmarkView {
    fn from(bm: &Bookmark) -> Self {
        BookmarkView {
            id: bm.id,
            URL: bm.URL.clone(),
            metadata: bm.metadata.clone(),
            tags: bm.tags.clone(),
            desc: bm.desc.clone(),
            flags: bm.flags,
            last_update_ts: bm.last_update_ts,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::adapter::json::bms_to_json;
    
    use camino::Utf8PathBuf;
    use rstest::*;

    use crate::adapter::dal::Dal;
    use crate::adapter::dal::migration::init_db;

    use super::*;

    #[fixture]
    fn bms() -> Vec<Bookmark> {
        let mut dal = Dal::new(String::from("../db/bkmr.db"));
        init_db(&mut dal.conn).expect("Error DB init");
        let bms = dal.get_bookmarks("");
        bms.unwrap()
    }

    #[fixture]
    fn test_data_path() -> Utf8PathBuf {
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
    }

    #[fixture]
    fn test_invalid_data_path() -> Utf8PathBuf {
        Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/invalid_data.ndjson")
    }

    // visual test: check that the JSON is printed to stdout
    #[rstest]
    fn test_bms_to_json(bms: Vec<Bookmark>) {
        bms_to_json(&bms);
    }

    #[test]
    fn check_json_format_valid_format() {
        let line = r#"{"id": "/a/b/readme.md:0", "content": "First record"}"#;
        assert!(check_json_format(line).is_ok());
    }

    #[test]
    fn check_json_format_missing_id() {
        let line = r#"{"content": "First record"}"#;
        assert!(check_json_format(line).is_err());
    }

    #[test]
    fn check_json_format_missing_content() {
        let line = r#"{"id": "/a/b/readme.md:0"}"#;
        assert!(check_json_format(line).is_err());
    }

    #[test]
    fn check_json_format_empty_string() {
        let line = "";
        assert!(check_json_format(line).is_err());
    }

    #[rstest]
    fn read_ndjson_file_and_create_bookmarks_valid_file(test_data_path: Utf8PathBuf) {
        let bookmarks = read_ndjson_file_and_create_bookmarks(test_data_path);
        assert!(bookmarks.is_ok());
        assert_eq!(bookmarks.unwrap().len(), 3);
    }

    #[rstest]
    fn read_ndjson_file_and_create_bookmarks_invalid_file(test_invalid_data_path: Utf8PathBuf) {
        let bookmarks = read_ndjson_file_and_create_bookmarks(test_invalid_data_path);
        println!("Bookmarks: {:?}", bookmarks);
        assert!(bookmarks.is_err());
    }

    #[test]
    fn read_ndjson_file_and_create_bookmarks_nonexistent_file() {
        let file_path = "test_data/nonexistent.ndjson";
        let bookmarks = read_ndjson_file_and_create_bookmarks(file_path);
        println!("Bookmarks: {:?}", bookmarks);
        assert!(bookmarks.is_err());
    }
}
