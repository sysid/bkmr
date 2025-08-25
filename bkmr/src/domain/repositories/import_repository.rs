// src/domain/repositories/import_repository.rs
use crate::domain::error::DomainResult;
use crate::domain::tag::Tag;
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::PathBuf;

pub struct BookmarkImportData {
    pub url: String,
    pub title: String,
    pub content: String,
    pub tags: HashSet<Tag>,
}

/// File import data with metadata for tracking file-based bookmarks
#[derive(Debug, Clone)]
pub struct FileImportData {
    pub name: String,         // Unique identifier from frontmatter
    pub tags: HashSet<Tag>,   // Tags from frontmatter
    pub content_type: String, // Type from frontmatter (default: _shell_)
    pub content: String,      // File content after frontmatter
    pub file_path: PathBuf,   // Source file path
    pub file_mtime: i64,      // File modification time (Unix timestamp)
    pub file_hash: String,    // SHA-256 hash of content
}

/// Options for file import behavior
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub update: bool,         // Update existing bookmarks when content differs
    pub delete_missing: bool, // Delete bookmarks whose source files no longer exist
    pub dry_run: bool,        // Show what would be done without making changes
    pub verbose: bool,        // Show detailed information about skipped files and validation issues
}

pub trait ImportRepository: Send + Sync + Debug {
    fn import_json_bookmarks(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>>;
    fn import_text_documents(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>>;
    fn import_files(
        &self,
        paths: &[String],
        options: &ImportOptions,
    ) -> DomainResult<Vec<FileImportData>>;
}
