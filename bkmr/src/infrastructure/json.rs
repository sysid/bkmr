// src/infrastructure/json.rs

use crate::application::dto::BookmarkResponse;
use crate::cli::error::{CliError, CliResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct TextDocument {
    id: String,
    content: String,
}

/// Reads a newline-delimited JSON (NDJSON) file and creates bookmarks.
/// Format: {"id": "/a/b/readme.md:0", "content": "First record"}
pub fn read_ndjson_file_and_create_bookmarks<P>(file_path: P) -> DomainResult<Vec<Bookmark>>
where
    P: AsRef<Path>,
{
    let file = File::open(file_path.as_ref())
        .map_err(|e| DomainError::CannotFetchMetadata(format!("Failed to open file: {}", e)))?;

    let reader = BufReader::new(file);
    let mut bookmarks = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| {
            DomainError::CannotFetchMetadata(format!("Failed to read line {}: {}", i + 1, e))
        })?;

        let record: TextDocument = serde_json::from_str(&line).map_err(|e| {
            DomainError::CannotFetchMetadata(format!(
                "Failed to parse JSON at line {}: {}",
                i + 1,
                e
            ))
        })?;

        let id = record.id;
        let filename = extract_filename(&id);

        let tags = Tag::parse_tags(",_imported_,")?;
        let bookmark = Bookmark::new(
            &id,             // URL
            &filename,       // Title
            &record.content, // Description
            tags,            // Tags
        )?;

        bookmarks.push(bookmark);
    }

    Ok(bookmarks)
}

/// Helper function to extract filename from a path
fn extract_filename(input: &str) -> String {
    use std::path::Path;

    // Attempt to split the input string by ':' to handle potential line indicators
    let parts: Vec<&str> = input.split(':').collect();
    let path_str = parts[0]; // The path part of the input

    // Use the Path type to manipulate file paths
    let path = Path::new(path_str);

    // Extract the filename, if it exists, and convert it to a String
    path.file_name().map_or(input.to_string(), |filename| {
        filename.to_string_lossy().to_string()
    })
}

#[derive(Serialize)]
pub struct BookmarkView {
    pub id: i32,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: String,
    pub access_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&Bookmark> for BookmarkView {
    fn from(bookmark: &Bookmark) -> Self {
        BookmarkView {
            id: bookmark.id().unwrap_or(0),
            url: bookmark.url().to_string(),
            title: bookmark.title().to_string(),
            description: bookmark.description().to_string(),
            tags: bookmark.formatted_tags(),
            access_count: bookmark.access_count(),
            created_at: bookmark.created_at().to_rfc3339(),
            updated_at: bookmark.updated_at().to_rfc3339(),
        }
    }
}

/// Converts bookmarks to JSON and writes to standard output
pub fn bms_to_json(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    let json = serde_json::to_string_pretty(&bookmarks).map_err(|e| {
        CliError::CommandFailed(format!("Failed to serialize bookmarks to JSON: {}", e))
    })?;

    io::stdout()
        .write_all(json.as_bytes())
        .map_err(|e| CliError::CommandFailed(format!("Failed to write JSON to stdout: {}", e)))?;

    println!();
    Ok(())
}
