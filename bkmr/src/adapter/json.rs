use crate::models::Bookmark;
use std::io;
use std::io::{BufRead, Write};
use camino::Utf8Path;
use std::fs::File;
use serde_derive::{Deserialize, Serialize};
use anyhow::Context;

pub fn bms_to_json(bms: &Vec<Bookmark>) {
    let json = serde_json::to_string_pretty(bms).expect("Failed to serialize bookmarks to JSON.");
    io::stdout()
        .write_all(json.as_bytes())
        .expect("Failed to write JSON to stdout.");
    println!();
}

// Define the Record struct to match the .ndjson file structure
#[derive(Serialize, Deserialize, Debug)]
pub struct Record {
    pub id: String,
    pub content: String,
}

// Function to read a .ndjson file and return a Vec<Record>
pub fn read_ndjson_file<P: AsRef<Utf8Path>>(file_path: P) -> anyhow::Result<Vec<Record>> {
    let file = File::open(file_path.as_ref()).with_context(|| format!("Failed to open file {:?}", file_path.as_ref()))?;
    let reader = io::BufReader::new(file);
    let mut records = Vec::new();

    for line in reader.lines() {
        let line = line.with_context(|| "Failed to read line from file")?;
        let record: Record = serde_json::from_str(&line)
            .with_context(|| format!("Failed to deserialize line: {}", line))?;
        records.push(record);
    }

    Ok(records)
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