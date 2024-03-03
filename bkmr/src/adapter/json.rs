use std::io;
use std::io::{BufRead, Write};
use camino::Utf8Path;
use std::fs::File;
use serde_derive::Serialize;
use anyhow::Context;
use chrono::NaiveDateTime;
use crate::helper::extract_filename;
use crate::model::bookmark::{Bookmark, BookmarkBuilder};

/// Reads a newline-delimited JSON (NDJSON) file and creates bookmarks from each line.
/// format: {"id": "/a/b/readme.md:0", "content": "First record"}
///
/// Mappings:
/// - `id` -> `URL`
/// - `id` -> `metadata` (filename)
/// - `content` -> `desc`
pub fn read_ndjson_file_and_create_bookmarks<P: AsRef<Utf8Path>>(file_path: P) -> anyhow::Result<Vec<Bookmark>> {
    let file = File::open(file_path.as_ref()).with_context(|| format!("Failed to open file {:?}", file_path.as_ref()))?;
    let reader = io::BufReader::new(file);
    let mut bookmarks = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| "Failed to read line from file")?;
        let record: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("Failed to deserialize line: {}", line))?;

        // todo: ensure exists and is uniq
        let id = record["id"].as_str().ok_or_else(|| anyhow::anyhow!("Invalid ID format"))?.to_string();
        let filename = extract_filename(&id);
        let mut bookmark = BookmarkBuilder::new()
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
    let json = serde_json::to_string_pretty(&bms_view).expect("Failed to serialize bookmarks to JSON.");
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
    use anyhow::anyhow;
    use rstest::*;
    use crate::adapter::json::bms_to_json;

    use crate::dal::Dal;
    use crate::helper::init_db;

    use super::*;

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

    #[fixture]
    fn bms() -> Vec<Bookmark> {
        let mut dal = Dal::new(String::from("../db/bkmr.db"));
        init_db(&mut dal.conn).expect("Error DB init");
        let bms = dal.get_bookmarks("");
        bms.unwrap()
    }

    // visual test: check that the JSON is printed to stdout
    #[rstest]
    fn test_bms_to_json(bms: Vec<Bookmark>) {
        bms_to_json(&bms);
    }
}