// src/infrastructure/repositories/json_import_repository.rs

use crate::domain::error::{DomainError, DomainResult, RepositoryError};
use crate::domain::repositories::import_repository::{
    BackupEntry, BookmarkImportData, ImportRepository,
};
use crate::domain::tag::Tag;
use chrono::{DateTime, Utc};
use crossterm::style::Stylize;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use tracing::{debug, warn};

#[derive(Deserialize)]
struct JsonBookmark {
    url: String,
    title: String,
    description: String,
    tags: Vec<String>,
}

/// The fields of `bkmr search --json` that a restore needs; serde skips the others
#[derive(Deserialize)]
struct JsonBackupEntry {
    id: Option<i32>,
    url: String,
    title: String,
    description: String,
    tags: Vec<String>,
    opener: Option<String>,
    embeddable: Option<bool>,
    file_path: Option<String>,
    created_at: Option<String>,
}

fn backup_error(name: &str, reason: String) -> DomainError {
    DomainError::RepositoryError(RepositoryError::Other(format!(
        "Backup entry {}: {}",
        name, reason
    )))
}

fn read_json_array<T: DeserializeOwned>(path: &str) -> DomainResult<Vec<T>> {
    let file = File::open(path)
        .map_err(|e| DomainError::CannotFetchMetadata(format!("Failed to open file: {}", e)))?;

    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content).map_err(|e| {
        DomainError::RepositoryError(RepositoryError::Other(format!(
            "Failed to read file content: {}",
            e
        )))
    })?;

    serde_json::from_str(&content).map_err(|e| {
        DomainError::RepositoryError(RepositoryError::Other(format!(
            "Failed to parse JSON: {}. Expected a JSON array of bookmark objects.",
            e
        )))
    })
}

#[derive(Debug)]
pub struct JsonImportRepository;

impl Default for JsonImportRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonImportRepository {
    pub fn new() -> Self {
        Self
    }
}

impl ImportRepository for JsonImportRepository {
    fn import_json_bookmarks(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>> {
        debug!(path = %path, "Importing bookmarks from JSON");
        let json_bookmarks: Vec<JsonBookmark> = read_json_array(path)?;

        let mut imports = Vec::new();

        for bookmark in json_bookmarks {
            let mut tags = HashSet::new();
            for tag_str in &bookmark.tags {
                match Tag::new(tag_str) {
                    // Unknown `_x_` tags are kept: the domain allows them, same as `bkmr add`.
                    Ok(tag) => {
                        tags.insert(tag);
                    }
                    Err(e) => {
                        warn!(tag = %tag_str, error = %e, "Invalid tag");
                        eprintln!("{} Invalid tag '{}': {}", "Warning".yellow(), tag_str, e);
                    }
                }
            }

            imports.push(BookmarkImportData {
                url: bookmark.url,
                title: bookmark.title,
                content: bookmark.description,
                tags,
            });
        }

        Ok(imports)
    }

    fn read_backup(&self, path: &str) -> DomainResult<Vec<BackupEntry>> {
        debug!(path = %path, "Reading backup from JSON");
        let json_entries: Vec<JsonBackupEntry> = read_json_array(path)?;

        json_entries
            .into_iter()
            .map(|entry| {
                let name = entry
                    .id
                    .map_or_else(|| format!("'{}'", entry.title), |id| format!("#{}", id));
                let tags = entry
                    .tags
                    .iter()
                    .map(|tag| Tag::new(tag))
                    .collect::<DomainResult<HashSet<Tag>>>()
                    .map_err(|e| backup_error(&name, format!("invalid tag: {}", e)))?;
                let created_at = entry
                    .created_at
                    .map(|value| {
                        DateTime::parse_from_rfc3339(&value)
                            .map(|dt| dt.with_timezone(&Utc))
                            .map_err(|e| {
                                backup_error(
                                    &name,
                                    format!("invalid created_at '{}': {}", value, e),
                                )
                            })
                    })
                    .transpose()?;
                Ok(BackupEntry {
                    id: entry.id,
                    url: entry.url,
                    title: entry.title,
                    description: entry.description,
                    tags,
                    opener: entry.opener,
                    embeddable: entry.embeddable,
                    file_path: entry.file_path,
                    created_at,
                })
            })
            .collect()
    }

    fn import_files(
        &self,
        _paths: &[String],
        _options: &crate::domain::repositories::import_repository::ImportOptions,
    ) -> DomainResult<Vec<crate::domain::repositories::import_repository::FileImportData>> {
        // JsonImportRepository doesn't handle file imports - delegate to FileImportRepository
        Err(DomainError::RepositoryError(
            crate::domain::error::RepositoryError::Other(
                "File import not supported by JsonImportRepository".to_string(),
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn given_json_array_file_when_import_bookmarks_then_creates_bookmark_data() -> DomainResult<()>
    {
        // Create a temporary test file with a JSON array
        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"[
                {{"url": "https://example.com/test", "title": "Test Entry", "description": "Test Description", "tags": ["test", "_imported_"]}},
                {{"url": "https://example.com/test2", "title": "Test Entry 2", "description": "Another Test", "tags": ["test2"]}}
            ]"#
        ).expect("Failed to write to temp file");

        // Create the repository
        let repo = JsonImportRepository::new();

        // Import the data
        let imports = repo.import_json_bookmarks(temp_file.path().to_str().unwrap())?;

        // Verify the results
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].url, "https://example.com/test");
        assert_eq!(imports[0].title, "Test Entry");
        assert_eq!(imports[0].content, "Test Description");
        assert!(imports[0].tags.iter().any(|t| t.value() == "test"));
        assert!(imports[0].tags.iter().any(|t| t.value() == "_imported_"));

        assert_eq!(imports[1].url, "https://example.com/test2");
        assert_eq!(imports[1].title, "Test Entry 2");
        assert_eq!(imports[1].content, "Another Test");
        assert_eq!(imports[1].tags.len(), 1);
        assert!(imports[1].tags.iter().any(|t| t.value() == "test2"));

        Ok(())
    }

    #[test]
    fn given_unknown_system_tag_when_import_bookmarks_then_keeps_tag_like_add() -> DomainResult<()>
    {
        // `bkmr add` keeps unknown `_x_` tags (the domain allows them), so import must too.
        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"[{{"url": "https://example.com/x", "title": "X", "description": "", "tags": ["foo", "_x_"]}}]"#
        )
        .expect("Failed to write to temp file");

        let repo = JsonImportRepository::new();
        let imports = repo.import_json_bookmarks(temp_file.path().to_str().unwrap())?;

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].tags.len(), 2);
        assert!(imports[0].tags.iter().any(|t| t.value() == "foo"));
        assert!(imports[0].tags.iter().any(|t| t.value() == "_x_"));

        Ok(())
    }

    #[test]
    fn given_search_json_export_when_reading_backup_then_keeps_id_and_restorable_fields(
    ) -> DomainResult<()> {
        // Shape written by `bkmr search --json`; fields restore ignores must not break parsing
        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"[{{"id": 12, "url": "echo v1", "title": "Snip", "description": "d",
                "created_at": "2026-01-02T03:04:05+00:00", "verified_at": null,
                "tags": ["_snip_", "demo"], "access_count": 3,
                "updated_at": "2026-02-01T00:00:00+00:00", "accessed_at": null,
                "opener": "vim {{}}", "embeddable": false, "has_embedding": false,
                "content_hash": null, "file_path": null, "file_mtime": null, "file_hash": null}}]"#
        )
        .expect("Failed to write to temp file");

        let entries =
            JsonImportRepository::new().read_backup(temp_file.path().to_str().unwrap())?;

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.id, Some(12));
        assert_eq!(entry.url, "echo v1");
        assert_eq!(entry.title, "Snip");
        assert_eq!(entry.description, "d");
        assert!(entry.tags.iter().any(|t| t.value() == "_snip_"));
        assert!(entry.tags.iter().any(|t| t.value() == "demo"));
        assert_eq!(entry.opener.as_deref(), Some("vim {}"));
        assert_eq!(entry.embeddable, Some(false));
        assert_eq!(entry.file_path, None);
        assert_eq!(
            entry.created_at.map(|dt| dt.to_rfc3339()),
            Some("2026-01-02T03:04:05+00:00".to_string())
        );
        Ok(())
    }

    #[test]
    fn given_hand_written_entry_without_id_when_reading_backup_then_id_and_optional_fields_are_none(
    ) -> DomainResult<()> {
        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"[{{"url": "https://example.com", "title": "T", "description": "", "tags": []}}]"#
        )
        .expect("Failed to write to temp file");

        let entries =
            JsonImportRepository::new().read_backup(temp_file.path().to_str().unwrap())?;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, None);
        assert_eq!(entries[0].embeddable, None);
        assert_eq!(entries[0].created_at, None);
        Ok(())
    }

    #[test]
    fn given_invalid_tag_when_reading_backup_then_fails_instead_of_dropping_the_tag() {
        // load-json warns and drops it; a restore that silently loses a tag is not a restore
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(
            temp_file,
            r#"[{{"id": 1, "url": "u", "title": "T", "description": "", "tags": ["has space"]}}]"#
        )
        .expect("Failed to write to temp file");

        let result = JsonImportRepository::new().read_backup(temp_file.path().to_str().unwrap());

        assert!(result.is_err());
    }

    #[test]
    fn given_invalid_created_at_when_reading_backup_then_fails() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(
            temp_file,
            r#"[{{"id": 1, "url": "u", "title": "T", "description": "", "tags": [],
                "created_at": "yesterday"}}]"#
        )
        .expect("Failed to write to temp file");

        let result = JsonImportRepository::new().read_backup(temp_file.path().to_str().unwrap());

        assert!(result.is_err());
    }

    #[test]
    fn given_ndjson_file_when_import_as_bookmarks_then_returns_error() {
        // Create a temporary test file with NDJSON format which should now fail
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{"url": "https://example.com/test", "title": "Test Entry", "description": "Test Description", "tags": ["test"]}}"#
        ).expect("Failed to write to temp file");
        writeln!(
            temp_file,
            r#"{{"url": "https://example.com/test2", "title": "Test Entry 2", "description": "Another Test", "tags": ["test2"]}}"#
        ).expect("Failed to write to temp file");

        // Create the repository
        let repo = JsonImportRepository::new();

        // Import should fail with an error since bookmarks should be in JSON array format
        let result = repo.import_json_bookmarks(temp_file.path().to_str().unwrap());
        assert!(result.is_err());
    }
}
