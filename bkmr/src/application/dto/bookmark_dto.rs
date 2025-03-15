use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;  // domain error alias
use crate::domain::tag::Tag;

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

// --------------------- DTO to Domain Conversions ---------------------

impl BookmarkCreateRequest {
    pub fn to_domain_objects(&self) -> DomainResult<(String, String, String, HashSet<Tag>)> {
        let mut tag_set = HashSet::new();
        if let Some(ref tag_strings) = self.tags {
            for tag_str in tag_strings {
                let parsed_tags = Tag::parse_tags(tag_str)?;
                tag_set.extend(parsed_tags);
            }
        }
        Ok((
            self.url.clone(),
            self.title.clone().unwrap_or_default(),
            self.description.clone().unwrap_or_default(),
            tag_set,
        ))
    }
}

impl BookmarkUpdateRequest {
    /// If tags exist, parse them into domain `Tag` objects.
    pub fn to_domain_tags(&self) -> DomainResult<Option<HashSet<Tag>>> {
        if let Some(ref tag_strings) = self.tags {
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
    /// Create a response DTO from a domain Bookmark
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

    /// Convert an entire collection of Bookmarks into response DTOs
    pub fn from_domain_collection(bookmarks: &[Bookmark]) -> Vec<Self> {
        bookmarks.iter().map(Self::from_domain).collect()
    }
}

impl BookmarkListItem {
    /// Create a list item DTO from a domain Bookmark
    pub fn from_domain(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id(),
            url: bookmark.url().to_string(),
            title: bookmark.title().to_string(),
            tags: bookmark.tags().iter().map(|t| t.value().to_string()).collect(),
        }
    }

    /// Convert a collection of Bookmarks into list item DTOs
    pub fn from_domain_collection(bookmarks: &[Bookmark]) -> Vec<Self> {
        bookmarks.iter().map(Self::from_domain).collect()
    }
}

impl BookmarkSearchRequest {
    /// Convert the search request into a more generic struct used in the application service,
    /// if desired (not mandatory if you build queries directly in the service).
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use std::collections::HashSet;

    #[test]
    fn test_bookmark_create_single_tag() {
        let request = BookmarkCreateRequest {
            url: "https://example.com".into(),
            title: Some("Example".into()),
            description: None,
            tags: Some(vec!["rust".into()]),
            fetch_metadata: None,
        };

        let (url, title, desc, tags) = request.to_domain_objects().unwrap();
        assert_eq!(url, "https://example.com");
        assert_eq!(title, "Example");
        assert_eq!(desc, ""); // no description given
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("rust").unwrap()));
    }

    #[test]
    fn test_bookmark_create_multiple_comma_separated_tags() {
        let request = BookmarkCreateRequest {
            url: "https://foo.com".into(),
            title: None,
            description: None,
            tags: Some(vec!["rust, programming, testing".into()]),
            fetch_metadata: None,
        };

        let (_, _, _, tags) = request.to_domain_objects().unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("rust").unwrap()));
        assert!(tags.contains(&Tag::new("programming").unwrap()));
        assert!(tags.contains(&Tag::new("testing").unwrap()));
    }

    #[test]
    fn test_bookmark_update_tags_none() {
        let request = BookmarkUpdateRequest {
            id: 42,
            title: Some("Updated Title".into()),
            description: None,
            tags: None,
        };

        let maybe_tags = request.to_domain_tags().unwrap();
        assert!(maybe_tags.is_none(), "No tags provided => None");
    }

    #[test]
    fn test_bookmark_update_with_tags() {
        let request = BookmarkUpdateRequest {
            id: 123,
            title: None,
            description: None,
            tags: Some(vec!["tag1".to_string(), "tag2, tag3".to_string()]),
        };

        let maybe_tags = request.to_domain_tags().unwrap();
        let tags = maybe_tags.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn test_bookmark_response_from_domain() {
        // Make a domain bookmark
        let mut tag_set = HashSet::new();
        tag_set.insert(Tag::new("rust").unwrap());
        tag_set.insert(Tag::new("programming").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Title",
            "Example Desc",
            tag_set,
        )
        .unwrap();

        // Convert to response
        let dto = BookmarkResponse::from_domain(&bookmark);

        assert_eq!(dto.url, "https://example.com");
        assert_eq!(dto.title, "Example Title");
        assert_eq!(dto.description, "Example Desc");
        assert_eq!(dto.tags.len(), 2);
        assert!(dto.tags.contains(&"rust".to_string()));
        assert!(dto.tags.contains(&"programming".to_string()));
    }

    #[test]
    fn test_bookmark_list_item_from_domain() {
        let bookmark = Bookmark::new(
            "https://testing.com",
            "TestTitle",
            "Testing Desc",
            HashSet::new(),
        )
        .unwrap();

        let item = BookmarkListItem::from_domain(&bookmark);
        assert_eq!(item.url, "https://testing.com");
        assert_eq!(item.title, "TestTitle");
        assert!(item.tags.is_empty());
    }
}