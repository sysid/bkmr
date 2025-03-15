use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::bookmark::Bookmark;
use crate::domain::tag::Tag;
use std::collections::HashSet;

/// Request object for creating a new bookmark
#[derive(Debug, Clone, Deserialize)]
pub struct BookmarkCreateRequest {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub fetch_metadata: Option<bool>,
}

/// Request object for updating an existing bookmark
#[derive(Debug, Clone, Deserialize)]
pub struct BookmarkUpdateRequest {
    pub id: i32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Response object for a bookmark
#[derive(Debug, Clone, Serialize)]
pub struct BookmarkResponse {
    pub id: Option<i32>,
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub access_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request object for searching bookmarks
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BookmarkSearchRequest {
    pub query: Option<String>,
    pub all_tags: Option<Vec<String>>,
    pub any_tags: Option<Vec<String>>,
    pub exclude_all_tags: Option<Vec<String>>,
    pub exclude_any_tags: Option<Vec<String>>,
    pub exact_tags: Option<Vec<String>>,
    pub sort_by_date: Option<bool>,
    pub sort_descending: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Simplified bookmark representation for lists
#[derive(Debug, Clone, Serialize)]
pub struct BookmarkListItem {
    pub id: Option<i32>,
    pub url: String,
    pub title: String,
    pub tags: Vec<String>,
}

/// Response object for bookmark search results
#[derive(Debug, Clone, Serialize)]
pub struct BookmarkSearchResponse {
    pub bookmarks: Vec<BookmarkListItem>,
    pub total_count: usize,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub has_more: bool,
}

/// Mapping functions for bookmark DTOs
impl BookmarkCreateRequest {
    /// Convert to domain entities
    pub fn to_domain_objects(&self) -> anyhow::Result<(String, String, String, HashSet<Tag>)> {
        let tags = if let Some(tag_strings) = &self.tags {
            let mut tag_set = HashSet::new();
            for tag_str in tag_strings {
                let parsed_tags = Tag::parse_tags(tag_str)?;
                tag_set.extend(parsed_tags);
            }
            tag_set
        } else {
            HashSet::new()
        };

        Ok((
            self.url.clone(),
            self.title.clone().unwrap_or_default(),
            self.description.clone().unwrap_or_default(),
            tags,
        ))
    }
}

impl BookmarkUpdateRequest {
    /// Convert to domain objects
    pub fn to_domain_tags(&self) -> anyhow::Result<Option<HashSet<Tag>>> {
        if let Some(tag_strings) = &self.tags {
            let mut tag_set = HashSet::new();
            for tag_str in tag_strings {
                let parsed_tags = Tag::parse_tags(tag_str)?;
                tag_set.extend(parsed_tags);
            }
            Ok(Some(tag_set))
        } else {
            Ok(None)
        }
    }
}

impl BookmarkResponse {
    /// Create from domain entity
    pub fn from_domain(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id(),
            url: bookmark.url().to_string(),
            title: bookmark.title().to_string(),
            description: bookmark.description().to_string(),
            tags: bookmark.tags().iter().map(|t| t.value().to_string()).collect(),
            access_count: bookmark.access_count(),
            created_at: bookmark.created_at(),
            updated_at: bookmark.updated_at(),
        }
    }

    /// Convert collection of bookmarks to responses
    pub fn from_domain_collection(bookmarks: &[Bookmark]) -> Vec<Self> {
        bookmarks.iter().map(Self::from_domain).collect()
    }
}

impl BookmarkListItem {
    /// Create from domain entity
    pub fn from_domain(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id(),
            url: bookmark.url().to_string(),
            title: bookmark.title().to_string(),
            tags: bookmark.tags().iter().map(|t| t.value().to_string()).collect(),
        }
    }

    /// Convert collection of bookmarks to list items
    pub fn from_domain_collection(bookmarks: &[Bookmark]) -> Vec<Self> {
        bookmarks.iter().map(Self::from_domain).collect()
    }
}

impl BookmarkSearchRequest {
    /// Convert to application search parameters
    pub fn to_application_params(&self) -> crate::application::services::search::SearchParamsDto {
        crate::application::services::search::SearchParamsDto {
            query: self.query.clone(),
            all_tags: self.all_tags.clone(),
            any_tags: self.any_tags.clone(),
            exclude_all_tags: self.exclude_all_tags.clone(),
            exclude_any_tags: self.exclude_any_tags.clone(),
            exact_tags: self.exact_tags.clone(),
            sort_by_date: self.sort_by_date,
            sort_descending: self.sort_descending,
            limit: self.limit,
            offset: self.offset,
        }
    }
}