use std::collections::HashSet;
use anyhow::Result;

use crate::domain::bookmark::Bookmark;
use crate::domain::tag::Tag;
use crate::domain::error::{DomainError, DomainResult};

/// Trait defining core bookmark operations
pub trait BookmarkService {
    /// Create a new bookmark
    fn create_bookmark(
        &self,
        url: &str,
        title: &str,
        description: &str,
        tags: HashSet<Tag>,
    ) -> DomainResult<Bookmark>;

    /// Update bookmark metadata
    fn update_bookmark_metadata(
        &self,
        bookmark: &mut Bookmark,
        title: &str,
        description: &str,
    ) -> DomainResult<()>;

    /// Add tags to a bookmark
    fn add_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()>;

    /// Remove tags from a bookmark
    fn remove_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()>;

    /// Replace all tags on a bookmark
    fn replace_tags(&self, bookmark: &mut Bookmark, tags: HashSet<Tag>) -> DomainResult<()>;

    /// Record bookmark access
    fn record_access(&self, bookmark: &mut Bookmark) -> DomainResult<()>;

    /// Filter bookmarks by tag criteria
    fn filter_by_tags(
        &self,
        bookmarks: &[Bookmark],
        all_tags: Option<HashSet<Tag>>,
        any_tags: Option<HashSet<Tag>>,
        all_tags_not: Option<HashSet<Tag>>,
        any_tags_not: Option<HashSet<Tag>>,
        exact_tags: Option<HashSet<Tag>>,
    ) -> Vec<Bookmark>;

    /// Search for bookmarks by full-text content
    fn search_by_content(&self, bookmarks: &[Bookmark], query: &str) -> Vec<Bookmark>;

    /// Extract metadata from a URL
    fn fetch_metadata(&self, url: &str) -> Result<(String, String, String)>;

    /// Generate content for embedding
    fn generate_embedding_content(&self, bookmark: &Bookmark) -> String;

    /// Calculate content hash for a bookmark
    fn calculate_content_hash(&self, bookmark: &Bookmark) -> Vec<u8>;
}

/// Implementation of BookmarkService
pub struct BookmarkServiceImpl;

impl BookmarkServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

impl BookmarkService for BookmarkServiceImpl {
    fn create_bookmark(
        &self,
        url: &str,
        title: &str,
        description: &str,
        tags: HashSet<Tag>,
    ) -> DomainResult<Bookmark> {
        Bookmark::new(url, title, description, tags)
    }

    fn update_bookmark_metadata(
        &self,
        bookmark: &mut Bookmark,
        title: &str,
        description: &str,
    ) -> DomainResult<()> {
        bookmark.update(title.to_string(), description.to_string());
        Ok(())
    }

    fn add_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()> {
        for tag in tags {
            bookmark.add_tag(tag.clone())?;
        }
        Ok(())
    }

    fn remove_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()> {
        let mut errors = Vec::new();

        for tag in tags {
            if let Err(e) = bookmark.remove_tag(tag) {
                errors.push(format!("{}", e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(DomainError::TagOperationFailed(
                format!("Failed to remove tags: {}", errors.join(", "))
            ))
        }
    }

    fn replace_tags(&self, bookmark: &mut Bookmark, tags: HashSet<Tag>) -> DomainResult<()> {
        bookmark.set_tags(tags)
    }

    fn record_access(&self, bookmark: &mut Bookmark) -> DomainResult<()> {
        bookmark.record_access();
        Ok(())
    }

    fn filter_by_tags(
        &self,
        bookmarks: &[Bookmark],
        all_tags: Option<HashSet<Tag>>,
        any_tags: Option<HashSet<Tag>>,
        all_tags_not: Option<HashSet<Tag>>,
        any_tags_not: Option<HashSet<Tag>>,
        exact_tags: Option<HashSet<Tag>>,
    ) -> Vec<Bookmark> {
        let mut filtered_bookmarks: Vec<Bookmark> = bookmarks.to_vec();

        // Filter by exact tags if specified
        if let Some(tags) = exact_tags {
            filtered_bookmarks.retain(|bm| bm.matches_exact_tags(&tags));
            return filtered_bookmarks;
        }

        // Apply all tag filters sequentially
        if let Some(tags) = all_tags {
            filtered_bookmarks.retain(|bm| bm.matches_all_tags(&tags));
        }

        if let Some(tags) = any_tags {
            filtered_bookmarks.retain(|bm| bm.matches_any_tag(&tags));
        }

        if let Some(tags) = all_tags_not {
            filtered_bookmarks.retain(|bm| !bm.matches_all_tags(&tags));
        }

        if let Some(tags) = any_tags_not {
            filtered_bookmarks.retain(|bm| !bm.matches_any_tag(&tags));
        }

        filtered_bookmarks
    }

    fn search_by_content(&self, bookmarks: &[Bookmark], query: &str) -> Vec<Bookmark> {
        if query.is_empty() {
            return bookmarks.to_vec();
        }

        let query = query.to_lowercase();
        bookmarks
            .iter()
            .filter(|bm| {
                let content = format!(
                    "{} {} {}",
                    bm.title().to_lowercase(),
                    bm.description().to_lowercase(),
                    bm.tags().iter().map(|t| t.value().to_lowercase()).collect::<Vec<_>>().join(" ")
                );
                content.contains(&query)
            })
            .cloned()
            .collect()
    }

    fn fetch_metadata(&self, _url: &str) -> Result<(String, String, String)> {
        // This is a domain services operation but delegates to external services
        // Actual implementation would use dependency injection for the external services
        // For now, we'll return empty values for simplicity
        Ok((String::new(), String::new(), String::new()))
    }

    fn generate_embedding_content(&self, bookmark: &Bookmark) -> String {
        bookmark.get_content_for_embedding()
    }

    fn calculate_content_hash(&self, bookmark: &Bookmark) -> Vec<u8> {
        let content = self.generate_embedding_content(bookmark);
        md5::compute(content).0.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bookmark() -> Bookmark {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("tag1").unwrap());
        tags.insert(Tag::new("tag2").unwrap());

        Bookmark::new(
            "https://example.com",
            "Example Site",
            "A test site",
            tags
        ).unwrap()
    }

    fn create_test_tags(tag_names: &[&str]) -> HashSet<Tag> {
        tag_names
            .iter()
            .map(|&name| Tag::new(name).unwrap())
            .collect()
    }

    #[test]
    fn test_create_bookmark() {
        let service = BookmarkServiceImpl::new();
        let tags = create_test_tags(&["test1", "test2"]);

        let bookmark = service.create_bookmark(
            "https://example.com",
            "Example",
            "Description",
            tags.clone()
        ).unwrap();

        assert_eq!(bookmark.url(), "https://example.com");
        assert_eq!(bookmark.title(), "Example");
        assert_eq!(bookmark.description(), "Description");
        assert_eq!(bookmark.tags().len(), 2);
    }

    #[test]
    fn test_update_bookmark_metadata() {
        let service = BookmarkServiceImpl::new();
        let mut bookmark = create_test_bookmark();

        service.update_bookmark_metadata(
            &mut bookmark,
            "Updated Title",
            "Updated Description"
        ).unwrap();

        assert_eq!(bookmark.title(), "Updated Title");
        assert_eq!(bookmark.description(), "Updated Description");
    }

    #[test]
    fn test_add_tags() {
        let service = BookmarkServiceImpl::new();
        let mut bookmark = create_test_bookmark();
        let new_tags = create_test_tags(&["tag3", "tag4"]);

        service.add_tags(&mut bookmark, &new_tags).unwrap();

        assert_eq!(bookmark.tags().len(), 4);
        assert!(bookmark.tags().contains(&Tag::new("tag3").unwrap()));
        assert!(bookmark.tags().contains(&Tag::new("tag4").unwrap()));
    }

    #[test]
    fn test_remove_tags() {
        let service = BookmarkServiceImpl::new();
        let mut bookmark = create_test_bookmark();
        let tags_to_remove = create_test_tags(&["tag1"]);

        service.remove_tags(&mut bookmark, &tags_to_remove).unwrap();

        assert_eq!(bookmark.tags().len(), 1);
        assert!(!bookmark.tags().contains(&Tag::new("tag1").unwrap()));
        assert!(bookmark.tags().contains(&Tag::new("tag2").unwrap()));
    }

    #[test]
    fn test_replace_tags() {
        let service = BookmarkServiceImpl::new();
        let mut bookmark = create_test_bookmark();
        let new_tags = create_test_tags(&["new1", "new2", "new3"]);

        service.replace_tags(&mut bookmark, new_tags).unwrap();

        assert_eq!(bookmark.tags().len(), 3);
        assert!(bookmark.tags().contains(&Tag::new("new1").unwrap()));
        assert!(bookmark.tags().contains(&Tag::new("new2").unwrap()));
        assert!(bookmark.tags().contains(&Tag::new("new3").unwrap()));
        assert!(!bookmark.tags().contains(&Tag::new("tag1").unwrap()));
    }

    #[test]
    fn test_record_access() {
        let service = BookmarkServiceImpl::new();
        let mut bookmark = create_test_bookmark();

        assert_eq!(bookmark.access_count(), 0);

        service.record_access(&mut bookmark).unwrap();
        assert_eq!(bookmark.access_count(), 1);

        service.record_access(&mut bookmark).unwrap();
        assert_eq!(bookmark.access_count(), 2);
    }

    #[test]
    fn test_filter_by_tags() {
        let service = BookmarkServiceImpl::new();

        // Create bookmarks with different tag combinations
        let mut bookmarks = Vec::new();

        // Bookmark 1: tag1, tag2
        bookmarks.push(create_test_bookmark());

        // Bookmark 2: tag2, tag3
        let tags2 = create_test_tags(&["tag2", "tag3"]);
        bookmarks.push(Bookmark::new(
            "https://example2.com", "Example 2", "Desc 2", tags2
        ).unwrap());

        // Bookmark 3: tag3, tag4
        let tags3 = create_test_tags(&["tag3", "tag4"]);
        bookmarks.push(Bookmark::new(
            "https://example3.com", "Example 3", "Desc 3", tags3
        ).unwrap());

        // Test ALL tags filter
        let result = service.filter_by_tags(
            &bookmarks,
            Some(create_test_tags(&["tag2"])),
            None, None, None, None
        );
        assert_eq!(result.len(), 2); // Bookmark 1 and 2 have tag2

        // Test ANY tags filter
        let result = service.filter_by_tags(
            &bookmarks,
            None,
            Some(create_test_tags(&["tag1", "tag4"])),
            None, None, None
        );
        assert_eq!(result.len(), 2); // Bookmark 1 has tag1, Bookmark 3 has tag4

        // Test ALL_NOT tags filter
        let result = service.filter_by_tags(
            &bookmarks,
            None, None,
            Some(create_test_tags(&["tag1"])),
            None, None
        );
        assert_eq!(result.len(), 2); // Bookmark 2 and 3 don't have tag1

        // Test ANY_NOT tags filter
        let result = service.filter_by_tags(
            &bookmarks,
            None, None, None,
            Some(create_test_tags(&["tag3"])),
            None
        );
        assert_eq!(result.len(), 1); // Only Bookmark 1 doesn't have tag3

        // Test EXACT tags filter
        let result = service.filter_by_tags(
            &bookmarks,
            None, None, None, None,
            Some(create_test_tags(&["tag3", "tag4"]))
        );
        assert_eq!(result.len(), 1); // Only Bookmark 3 has exactly tag3 and tag4
    }

    #[test]
    fn test_search_by_content() {
        let service = BookmarkServiceImpl::new();

        // Create bookmarks with different content
        let mut bookmarks = Vec::new();

        // Bookmark 1: contains "rust" in title
        let tags1 = create_test_tags(&["programming"]);
        bookmarks.push(Bookmark::new(
            "https://rust-lang.org", "Rust Language", "A programming language", tags1
        ).unwrap());

        // Bookmark 2: contains "rust" in description
        let tags2 = create_test_tags(&["language"]);
        bookmarks.push(Bookmark::new(
            "https://example.com", "Example", "About Rust programming", tags2
        ).unwrap());

        // Bookmark 3: contains "rust" in tags
        let tags3 = create_test_tags(&["rust", "code"]);
        bookmarks.push(Bookmark::new(
            "https://example.org", "Example Org", "A website", tags3
        ).unwrap());

        // Bookmark 4: doesn't contain "rust" anywhere
        let tags4 = create_test_tags(&["unrelated"]);
        bookmarks.push(Bookmark::new(
            "https://example.net", "Something Else", "Unrelated content", tags4
        ).unwrap());

        // Search for "rust"
        let results = service.search_by_content(&bookmarks, "rust");
        assert_eq!(results.len(), 3); // Should find bookmarks 1, 2, and 3

        // Search for "programming"
        let results = service.search_by_content(&bookmarks, "programming");
        assert_eq!(results.len(), 2); // Should find bookmarks 1 and 2

        // Search for "unrelated"
        let results = service.search_by_content(&bookmarks, "unrelated");
        assert_eq!(results.len(), 1); // Should find only bookmark 4

        // Empty search should return all bookmarks
        let results = service.search_by_content(&bookmarks, "");
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_calculate_content_hash() {
        let service = BookmarkServiceImpl::new();
        let bookmark = create_test_bookmark();

        let hash1 = service.calculate_content_hash(&bookmark);
        assert!(!hash1.is_empty());

        // Create a clone with the same content
        let bookmark_clone = create_test_bookmark();
        let hash2 = service.calculate_content_hash(&bookmark_clone);

        // Hashes should be the same for identical content
        assert_eq!(hash1, hash2);
    }
}