use std::collections::HashSet;
use std::fmt;
use chrono::{DateTime, Utc};
use url::Url;

use crate::domain::error::{DomainError, DomainResult};
use crate::domain::tag::Tag;

/// Represents a bookmark domain entity
#[derive(Debug, Clone, PartialEq)]
pub struct Bookmark {
    id: Option<i32>,
    url: String,
    title: String,
    description: String,
    tags: HashSet<Tag>,
    access_count: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Bookmark {
    /// Creates a new bookmark with validation
    pub fn new<S: AsRef<str>>(
        url: S,
        title: S,
        description: S,
        tags: HashSet<Tag>,
    ) -> DomainResult<Self> {
        let url_str = url.as_ref();

        // Validate URL unless it's a special URL like "shell::" or a file path
        if !url_str.starts_with("shell::") && !url_str.starts_with('/') && !url_str.starts_with('~') && Url::parse(url_str).is_err() {
            return Err(DomainError::InvalidUrl(url_str.to_string()));
        }

        let now = Utc::now();

        Ok(Self {
            id: None,
            url: url_str.to_string(),
            title: title.as_ref().to_string(),
            description: description.as_ref().to_string(),
            tags,
            access_count: 0,
            created_at: now,
            updated_at: now,
        })
    }

    /// Creates a bookmark from existing data (typically from storage)
    pub fn from_storage(
        id: i32,
        url: String,
        title: String,
        description: String,
        tag_string: String,
        access_count: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let tags = Tag::parse_tags(tag_string)?;

        Ok(Self {
            id: Some(id),
            url,
            title,
            description,
            tags,
            access_count,
            created_at,
            updated_at,
        })
    }

    // Getters
    pub fn id(&self) -> Option<i32> { self.id }
    pub fn url(&self) -> &str { &self.url }
    pub fn title(&self) -> &str { &self.title }
    pub fn description(&self) -> &str { &self.description }
    pub fn tags(&self) -> &HashSet<Tag> { &self.tags }
    pub fn access_count(&self) -> i32 { self.access_count }
    pub fn created_at(&self) -> DateTime<Utc> { self.created_at }
    pub fn updated_at(&self) -> DateTime<Utc> { self.updated_at }

    // Domain operations

    /// Add a tag to the bookmark
    pub fn add_tag(&mut self, tag: Tag) -> DomainResult<()> {
        self.tags.insert(tag);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Remove a tag from the bookmark
    pub fn remove_tag(&mut self, tag: &Tag) -> DomainResult<()> {
        if !self.tags.remove(tag) {
            return Err(DomainError::TagOperationFailed(
                format!("Tag '{}' not found on bookmark", tag)
            ));
        }

        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set all tags at once (replacing existing tags)
    pub fn set_tags(&mut self, tags: HashSet<Tag>) -> DomainResult<()> {
        self.tags = tags;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Record access to the bookmark
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.updated_at = Utc::now();
    }

    /// Update bookmark information
    pub fn update(&mut self, title: String, description: String) {
        self.title = title;
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// Get formatted tag string in the format ",tag1,tag2,"
    pub fn formatted_tags(&self) -> String {
        Tag::format_tags(&self.tags)
    }

    /// Get the content for embedding generation
    pub fn get_content_for_embedding(&self) -> String {
        // Filter out system tags (starting or ending with underscore)
        let visible_tags: HashSet<_> = self.tags.iter()
            .filter(|tag| !tag.value().starts_with('_') && !tag.value().ends_with('_'))
            .cloned()
            .collect();

        let tags_str = Tag::format_tags(&visible_tags);
        format!("{}{} -- {}{}", tags_str, self.title, self.description, tags_str)
    }

    /// Check if the bookmark matches all given tags
    pub fn matches_all_tags(&self, tags: &HashSet<Tag>) -> bool {
        Tag::contains_all(&self.tags, tags)
    }

    /// Check if the bookmark matches any of the given tags
    pub fn matches_any_tag(&self, tags: &HashSet<Tag>) -> bool {
        Tag::contains_any(&self.tags, tags)
    }

    /// Check if the bookmark has exactly the given tags
    pub fn matches_exact_tags(&self, tags: &HashSet<Tag>) -> bool {
        self.tags == *tags
    }

    /// Set the ID (typically used after storage)
    pub fn set_id(&mut self, id: i32) {
        self.id = Some(id);
    }
}

impl fmt::Display for Bookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {} ({})",
            self.id.map_or("New".to_string(), |id| id.to_string()),
            self.title,
            self.url,
            Tag::format_tags(&self.tags)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bookmark_valid() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        assert_eq!(bookmark.url(), "https://example.com");
        assert_eq!(bookmark.title(), "Example Site");
        assert_eq!(bookmark.description(), "An example website");
        assert_eq!(bookmark.tags().len(), 1);
        assert!(bookmark.tags().contains(&Tag::new("test").unwrap()));
        assert_eq!(bookmark.access_count(), 0);
    }

    #[test]
    fn test_new_bookmark_invalid_url() {
        let tags = HashSet::new();

        let result = Bookmark::new(
            "not-a-valid-url",
            "Invalid URL Test",
            "Testing invalid URL handling",
            tags
        );

        assert!(result.is_err());
        match result {
            Err(DomainError::InvalidUrl(url)) => {
                assert_eq!(url, "not-a-valid-url");
            },
            _ => panic!("Expected InvalidUrl error")
        }
    }

    #[test]
    fn test_special_urls_are_valid() {
        let tags = HashSet::new();

        // Shell command URL
        let shell_url = Bookmark::new(
            "shell::echo hello",
            "Shell Command",
            "A shell command",
            tags.clone()
        );
        assert!(shell_url.is_ok());

        // File path URL
        let file_url = Bookmark::new(
            "/path/to/file.txt",
            "File Path",
            "A file path",
            tags.clone()
        );
        assert!(file_url.is_ok());

        // Home directory path
        let home_url = Bookmark::new(
            "~/documents/file.txt",
            "Home Path",
            "A path in home directory",
            tags
        );
        assert!(home_url.is_ok());
    }

    #[test]
    fn test_add_remove_tags() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        // Add a tag
        bookmark.add_tag(Tag::new("added").unwrap()).unwrap();
        assert_eq!(bookmark.tags().len(), 2);
        assert!(bookmark.tags().contains(&Tag::new("added").unwrap()));

        // Remove a tag
        bookmark.remove_tag(&Tag::new("initial").unwrap()).unwrap();
        assert_eq!(bookmark.tags().len(), 1);
        assert!(!bookmark.tags().contains(&Tag::new("initial").unwrap()));

        // Try to remove a non-existent tag
        let result = bookmark.remove_tag(&Tag::new("nonexistent").unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_set_tags() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("initial").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        // Set completely new tags
        let mut new_tags = HashSet::new();
        new_tags.insert(Tag::new("new1").unwrap());
        new_tags.insert(Tag::new("new2").unwrap());

        bookmark.set_tags(new_tags.clone()).unwrap();
        assert_eq!(bookmark.tags(), &new_tags);
        assert_eq!(bookmark.tags().len(), 2);
    }

    #[test]
    fn test_record_access() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        assert_eq!(bookmark.access_count(), 0);

        bookmark.record_access();
        assert_eq!(bookmark.access_count(), 1);

        bookmark.record_access();
        assert_eq!(bookmark.access_count(), 2);
    }

    #[test]
    fn test_update() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        let old_updated_at = bookmark.updated_at();

        // Small delay to ensure updated_at changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        bookmark.update(
            "Updated Title".to_string(),
            "Updated description".to_string()
        );

        assert_eq!(bookmark.title(), "Updated Title");
        assert_eq!(bookmark.description(), "Updated description");
        assert!(bookmark.updated_at() > old_updated_at);
    }

    #[test]
    fn test_formatted_tags() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("tag1").unwrap());
        tags.insert(Tag::new("tag2").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        let formatted = bookmark.formatted_tags();
        assert!(formatted == ",tag1,tag2," || formatted == ",tag2,tag1,");
    }

    #[test]
    fn test_get_content_for_embedding() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("visible").unwrap());
        tags.insert(Tag::new("_system").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            tags
        ).unwrap();

        let content = bookmark.get_content_for_embedding();
        assert!(content.contains("visible"));
        assert!(!content.contains("_system"));
        assert!(content.contains("Example Site"));
        assert!(content.contains("An example website"));
    }

    #[test]
    fn test_tag_matching() {
        let mut bookmark_tags = HashSet::new();
        bookmark_tags.insert(Tag::new("tag1").unwrap());
        bookmark_tags.insert(Tag::new("tag2").unwrap());
        bookmark_tags.insert(Tag::new("tag3").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "An example website",
            bookmark_tags
        ).unwrap();

        // Test matches_all_tags
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());

        assert!(bookmark.matches_all_tags(&query_tags));

        query_tags.insert(Tag::new("tag4").unwrap());
        assert!(!bookmark.matches_all_tags(&query_tags));

        // Test matches_any_tag
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag4").unwrap());

        assert!(bookmark.matches_any_tag(&query_tags));

        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag4").unwrap());
        query_tags.insert(Tag::new("tag5").unwrap());

        assert!(!bookmark.matches_any_tag(&query_tags));

        // Test matches_exact_tags
        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());
        query_tags.insert(Tag::new("tag3").unwrap());

        assert!(bookmark.matches_exact_tags(&query_tags));

        let mut query_tags = HashSet::new();
        query_tags.insert(Tag::new("tag1").unwrap());
        query_tags.insert(Tag::new("tag2").unwrap());

        assert!(!bookmark.matches_exact_tags(&query_tags));
    }
}