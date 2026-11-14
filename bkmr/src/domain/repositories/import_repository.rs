// src/domain/repositories/import_repository.rs
use crate::domain::error::DomainResult;
use crate::domain::tag::Tag;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::PathBuf;

pub struct BookmarkImportData {
    pub url: String,
    pub title: String,
    pub content: String,
    pub tags: HashSet<Tag>,
}

/// One bookmark as bkmr's own JSON export (`bkmr search --json`) wrote it, read back for a
/// restore. `id` is what ties it to a row: content can change, an ID never means another bookmark.
#[derive(Debug, Clone)]
pub struct BackupEntry {
    /// `None` for an entry that bkmr did not export, e.g. a hand-written `load-json` file
    pub id: Option<i32>,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: HashSet<Tag>,
    pub opener: Option<String>,
    /// `None` if the export predates the field
    pub embeddable: Option<bool>,
    /// Set for a bookmark imported from a file, which then decides its content
    pub file_path: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
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
    /// Read a JSON export for `restore --from`. Fails on any entry it cannot take as written,
    /// since a restore must bring back exactly what was saved.
    fn read_backup(&self, path: &str) -> DomainResult<Vec<BackupEntry>>;
    fn import_files(
        &self,
        paths: &[String],
        options: &ImportOptions,
    ) -> DomainResult<Vec<FileImportData>>;
    /// Read one source file, as `import_files` reads each file it finds
    fn read_file(&self, path: &std::path::Path) -> DomainResult<FileImportData>;
}
