// src/domain/repositories/import_repository.rs
use crate::domain::error::DomainResult;
use crate::domain::tag::Tag;
use std::collections::HashSet;
use std::fmt::Debug;

pub struct BookmarkImportData {
    pub url: String,
    pub title: String,
    pub content: String,
    pub tags: HashSet<Tag>,
}

pub trait ImportRepository: Send + Sync + Debug {
    fn import_json_bookmarks(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>>;
    fn import_text_documents(&self, path: &str) -> DomainResult<Vec<BookmarkImportData>>;
}
