// src/infrastructure/json.rs

use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::tag::Tag;
use crate::util::path::extract_filename;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
struct TextDocument {
    id: String,
    content: String,
}

/// Structure for serializing bookmarks to JSON output
#[derive(Serialize)]
pub struct JsonBookmarkView {
    pub id: Option<i32>,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub access_count: i32,
    pub created_at: Option<String>,
    pub updated_at: String,
}

impl JsonBookmarkView {
    /// Create from a domain `Bookmark`
    pub fn from_domain(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id,
            url: bookmark.url.to_string(),
            title: bookmark.title.to_string(),
            description: bookmark.description.to_string(),
            tags: bookmark
                .tags
                .iter()
                .map(|tag| tag.value().to_string())
                .collect(),
            access_count: bookmark.access_count,
            created_at: bookmark.created_at.map(|dt| dt.to_rfc3339()),
            updated_at: bookmark.updated_at.to_rfc3339(),
        }
    }

    /// Convert a slice of bookmarks into a vector of JSON views
    pub fn from_domain_collection(bookmarks: &[Bookmark]) -> Vec<Self> {
        bookmarks.iter().map(Self::from_domain).collect()
    }
}

/// Converts bookmarks to JSON and writes to standard output
/// todo: add flag for embeddings
/// Standard output is used for pipeable content without colors or formatting
pub fn write_bookmarks_as_json(views: &[JsonBookmarkView]) -> DomainResult<()> {
    let json = serde_json::to_string_pretty(&views).map_err(|e| {
        DomainError::BookmarkOperationFailed(format!(
            "Failed to serialize bookmarks to JSON: {}",
            e
        ))
    })?;

    println!("{}", json);

    // Flush stdout to ensure immediate output
    std::io::stdout().flush().map_err(|e| {
        DomainError::BookmarkOperationFailed(format!("Failed to flush stdout: {}", e))
    })?;

    Ok(())
}

/// Checks the format of a JSON string.
///
/// Validates that the JSON contains required fields: "id" and "content"
fn check_json_format(line: &str) -> DomainResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| DomainError::CannotFetchMetadata(format!("Invalid JSON: {}", e)))?;

    if record["id"].is_null() || record["content"].is_null() {
        return Err(DomainError::CannotFetchMetadata(
            "Missing required fields (id, content)".to_string(),
        ));
    }

    Ok(())
}

/// Reads a newline-delimited JSON (NDJSON) file and creates bookmarks.
///
/// Format: {"id": "/a/b/readme.md:0", "content": "First record"}
///
/// Mappings:
/// - `id` -> URL
/// - Filename from `id` -> Title
/// - `content` -> Description
/// - "_imported_" tag added to all bookmarks
#[allow(dead_code)]
pub fn read_ndjson_file_and_create_bookmarks<P>(file_path: P) -> DomainResult<Vec<Bookmark>>
where
    P: AsRef<Path> + std::fmt::Display,
{
    let file = File::open(file_path.as_ref())
        .map_err(|e| DomainError::CannotFetchMetadata(format!("Failed to open file: {}", e)))?;

    let reader = BufReader::new(file);
    let mut bookmarks = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| {
            DomainError::CannotFetchMetadata(format!("Failed to read line {}: {}", i + 1, e))
        })?;

        check_json_format(&line)?;

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
        let dummy_embedder = crate::infrastructure::embeddings::DummyEmbedding;
        let bookmark = Bookmark::new(
            &id,             // URL
            &filename,       // Title
            &record.content, // Description
            tags,            // Tags
            &dummy_embedder,  // TODO: check whether the real embedder is required (looks like it from before refactor)
        )?;

        bookmarks.push(bookmark);
    }

    Ok(bookmarks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, EnvGuard};
    use std::collections::HashSet;
    use tempfile::NamedTempFile;

    #[test]
    fn given_valid_json_file_when_check_format_then_returns_true() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();
        let line = r#"{"id": "/a/b/readme.md:0", "content": "First record"}"#;
        assert!(check_json_format(line).is_ok());
    }

    #[test]
    fn given_invalid_json_file_when_check_format_then_returns_false() {
        let _ = init_test_env();
        let _guard = EnvGuard::new();
        let line = r#"{"id": "/a/b/readme.md:0"}"#; // Missing content
        assert!(check_json_format(line).is_err());

        let line = r#"{"content": "First record"}"#; // Missing id
        assert!(check_json_format(line).is_err());

        let line = "not json";
        assert!(check_json_format(line).is_err());
    }

    #[test]
    fn given_ndjson_file_when_read_then_creates_bookmarks() -> DomainResult<()> {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        // Create a temporary test file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"{{"id": "/path/to/file1.md:0", "content": "Content 1"}}"#
        )
        .expect("Failed to write to temp file");
        writeln!(
            temp_file,
            r#"{{"id": "/path/to/file2.md:0", "content": "Content 2"}}"#
        )
        .expect("Failed to write to temp file");

        // Read the bookmarks from the file
        let bookmarks = read_ndjson_file_and_create_bookmarks(temp_file.path().to_str().unwrap())?;

        // Verify the result
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].url, "/path/to/file1.md:0");
        assert_eq!(bookmarks[0].title, "file1.md");
        assert_eq!(bookmarks[0].description, "Content 1");
        assert!(bookmarks[0].tags.iter().any(|t| t.value() == "_imported_"));

        assert_eq!(bookmarks[1].url, "/path/to/file2.md:0");
        assert_eq!(bookmarks[1].title, "file2.md");
        assert_eq!(bookmarks[1].description, "Content 2");

        Ok(())
    }

    #[test]
    #[ignore = "This is a visual test that would output to stdout"]
    fn given_bookmarks_when_write_as_json_then_creates_valid_file() -> DomainResult<()> {
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        // Create a test bookmark
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test")?);

        let dummy_embedder = crate::infrastructure::embeddings::DummyEmbedding;
        let bookmark = Bookmark::new(
            "https://example.com",
            "Example",
            "A test bookmark",
            tags,
            &dummy_embedder,
        )?;

        // Convert to JSON views
        let views = vec![JsonBookmarkView::from_domain(&bookmark)];

        // This is a visual test that would output to stdout
        // We're just checking that it executes without errors
        write_bookmarks_as_json(&views)?;
        Ok(())
    }
}
