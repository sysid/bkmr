// src/infrastructure/repositories/json_import_repository.rs

use crate::domain::error::{DomainError, DomainResult, RepositoryError};
use crate::domain::repositories::import_repository::{BookmarkImportData, ImportRepository};
use crate::domain::tag::Tag;
use crate::util::path::extract_filename;
use crossterm::style::Stylize;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};

#[derive(Deserialize)]
struct TextDocument {
    id: String,
    content: String,
}

#[derive(Deserialize)]
struct JsonBookmark {
    url: String,
    title: String,
    description: String,
    tags: Vec<String>,
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

    /// Checks if the text document JSON has the required fields
    fn check_text_document_format(&self, line: &str) -> DomainResult<()> {
        let record: serde_json::Value = serde_json::from_str(line)
            .map_err(|e| DomainError::CannotFetchMetadata(format!("Invalid JSON: {}", e)))?;

        if record["id"].is_null() || record["content"].is_null() {
            return Err(DomainError::CannotFetchMetadata(
                "Missing required fields (id, content)".to_string(),
            ));
        }

        Ok(())
    }
}

impl ImportRepository for JsonImportRepository {
    fn import_json_bookmarks(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>> {
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

        // Parse as a JSON array
        let json_bookmarks: Vec<JsonBookmark> = serde_json::from_str(&content).map_err(|e| {
            DomainError::RepositoryError(RepositoryError::Other(format!(
                "Failed to parse JSON: {}. Expected a JSON array of bookmark objects.",
                e
            )))
        })?;

        let mut imports = Vec::new();

        for bookmark in json_bookmarks {
            let mut tags = HashSet::new();
            for tag_str in &bookmark.tags {
                match Tag::new(tag_str) {
                    Ok(tag) => {
                        if tag.is_system_tag() && !tag.is_known_system_tag() {
                            eprintln!(
                                "{} Unknown system tag '{}' ignored",
                                "Warning".yellow(),
                                tag.value()
                            );
                            continue;
                        }
                        tags.insert(tag);
                    }
                    Err(e) => {
                        // Log warning but continue
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

    fn import_text_documents(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>> {
        let file = File::open(path)
            .map_err(|e| DomainError::CannotFetchMetadata(format!("Failed to open file: {}", e)))?;

        let reader = BufReader::new(file);
        let mut imports = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                DomainError::RepositoryError(RepositoryError::Other(format!(
                    "Failed to read line {}: {}",
                    i + 1,
                    e
                )))
            })?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Validate the document structure
            self.check_text_document_format(&line)?;

            let record: TextDocument = serde_json::from_str(&line).map_err(|e| {
                DomainError::RepositoryError(RepositoryError::Other(format!(
                    "Failed to parse JSON at line {}: {}",
                    i + 1,
                    e
                )))
            })?;

            let id = record.id;
            let filename = extract_filename(&id);
            let tags = Tag::parse_tags(",_imported_,")?;

            imports.push(BookmarkImportData {
                url: id,
                title: filename,
                content: record.content,
                tags,
            });
        }

        Ok(imports)
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
    fn given_json_array_file_when_import_bookmarks_then_creates_bookmark_data() -> DomainResult<()> {
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

    #[test]
    fn given_ndjson_file_when_import_text_documents_then_creates_document_data() -> DomainResult<()> {
        // Create temporary test file with NDJSON format
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"{{"id": "doc1", "content": "Document 1 content."}}"#
        )?;
        writeln!(
            temp_file,
            r#"{{"id": "doc2", "content": "Document 2 content."}}"#
        )?;

        let repo = JsonImportRepository::new();
        let imports = repo.import_text_documents(temp_file.path().to_str().unwrap())?;

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].url, "doc1");
        assert_eq!(imports[0].title, "doc1");
        assert_eq!(imports[0].content, "Document 1 content.");
        assert!(imports[0].tags.iter().any(|t| t.value() == "_imported_"));

        assert_eq!(imports[1].url, "doc2");
        assert_eq!(imports[1].title, "doc2");
        assert_eq!(imports[1].content, "Document 2 content.");
        assert!(imports[1].tags.iter().any(|t| t.value() == "_imported_"));

        Ok(())
    }

    #[test]
    fn given_json_array_when_import_as_text_documents_then_returns_error() {
        // Create temporary test file with JSON array format
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(
            temp_file,
            r#"[
                {{"id": "doc1", "content": "Document 1 content."}},
                {{"id": "doc2", "content": "Document 2 content."}}
            ]"#
        )
        .unwrap();

        let repo = JsonImportRepository::new();
        let result = repo.import_text_documents(temp_file.path().to_str().unwrap());
        assert!(
            result.is_err(),
            "Should fail when trying to parse JSON array as NDJSON"
        );
    }

    #[test]
    fn given_invalid_json_when_import_text_documents_then_returns_error() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, r#"{{"id": "doc1" invalid json}}"#).unwrap();

        let repo = JsonImportRepository::new();
        let result = repo.import_text_documents(temp_file.path().to_str().unwrap());
        assert!(result.is_err());
    }
}
