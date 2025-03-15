use std::collections::{HashMap, HashSet};

use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::tag::Tag;

/// Trait defining core tag operations
pub trait TagService {
    /// Create a new tag with validation
    fn create_tag(&self, name: &str) -> DomainResult<Tag>;

    /// Parse a tag string into a set of validated tags
    fn parse_tag_string(&self, tag_string: &str) -> DomainResult<HashSet<Tag>>;

    /// Format a set of tags into a normalized tag string
    fn format_tags(&self, tags: &HashSet<Tag>) -> String;

    /// Check if a tag is valid
    fn is_valid_tag(&self, name: &str) -> bool;

    /// Normalize a single tag name
    fn normalize_tag_name(&self, name: &str) -> String;

    /// Find all tags used across a set of bookmarks
    fn find_all_tags(&self, bookmarks: &[Bookmark]) -> HashSet<Tag>;

    /// Calculate tag frequency across bookmarks
    fn calculate_tag_frequency(&self, bookmarks: &[Bookmark]) -> HashMap<Tag, usize>;

    /// Find related tags (tags that co-occur with the given tag)
    fn find_related_tags(&self, tag: &Tag, bookmarks: &[Bookmark]) -> HashMap<Tag, usize>;

    /// Merge two tags (replace one tag with another across all bookmarks)
    fn merge_tags(
        &self,
        bookmarks: &mut [Bookmark],
        source: &Tag,
        target: &Tag,
    ) -> DomainResult<usize>;

    /// Rename a tag across all bookmarks
    fn rename_tag(
        &self,
        bookmarks: &mut [Bookmark],
        old_tag: &Tag,
        new_name: &str,
    ) -> DomainResult<usize>;

    /// Check if a tag set is different from bookmarks current tags
    fn has_tag_changes(&self, bookmark: &Bookmark, new_tags: &HashSet<Tag>) -> bool;

    /// Replace all tags on a bookmark
    fn replace_tags(&self, bookmark: &mut Bookmark, tags: HashSet<Tag>) -> DomainResult<()>;

    /// Add tags to a bookmark
    fn add_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()>;

    /// Remove tags from a bookmark
    fn remove_tags(&self, bookmark: &mut Bookmark, tags: &HashSet<Tag>) -> DomainResult<()>;
}

/// Implementation of TagService
pub struct TagServiceImpl;

impl TagServiceImpl {
    pub fn new() -> Self {
        Self {}
    }

    /// Helper function to filter system tags
    fn is_system_tag(&self, tag: &Tag) -> bool {
        let value = tag.value();
        value.starts_with('_') || value.ends_with('_')
    }
}

impl TagService for TagServiceImpl {
    fn create_tag(&self, name: &str) -> DomainResult<Tag> {
        Tag::new(name)
    }

    fn parse_tag_string(&self, tag_string: &str) -> DomainResult<HashSet<Tag>> {
        Tag::parse_tags(tag_string)
    }

    fn format_tags(&self, tags: &HashSet<Tag>) -> String {
        Tag::format_tags(tags)
    }

    fn is_valid_tag(&self, name: &str) -> bool {
        let name = self.normalize_tag_name(name);

        if name.is_empty() {
            return false;
        }

        !name.contains(',') && !name.contains(' ')
    }

    fn normalize_tag_name(&self, name: &str) -> String {
        name.trim().to_lowercase()
    }

    fn find_all_tags(&self, bookmarks: &[Bookmark]) -> HashSet<Tag> {
        let mut all_tags = HashSet::new();

        for bookmark in bookmarks {
            for tag in bookmark.tags() {
                all_tags.insert(tag.clone());
            }
        }

        all_tags
    }

    fn calculate_tag_frequency(&self, bookmarks: &[Bookmark]) -> HashMap<Tag, usize> {
        let mut frequency = HashMap::new();

        for bookmark in bookmarks {
            for tag in bookmark.tags() {
                *frequency.entry(tag.clone()).or_insert(0) += 1;
            }
        }

        frequency
    }

    fn find_related_tags(&self, tag: &Tag, bookmarks: &[Bookmark]) -> HashMap<Tag, usize> {
        let mut related = HashMap::new();

        // Find bookmarks that have the specified tag
        let filtered_bookmarks: Vec<_> = bookmarks
            .iter()
            .filter(|bm| bm.tags().contains(tag))
            .collect();

        // Count occurrences of other tags in these bookmarks
        for bookmark in filtered_bookmarks {
            for other_tag in bookmark.tags() {
                if other_tag != tag {
                    *related.entry(other_tag.clone()).or_insert(0) += 1;
                }
            }
        }

        related
    }

    fn merge_tags(
        &self,
        bookmarks: &mut [Bookmark],
        source: &Tag,
        target: &Tag,
    ) -> DomainResult<usize> {
        if source == target {
            return Err(DomainError::TagOperationFailed(
                "Source and target tags are the same".to_string(),
            ));
        }

        let mut count = 0;

        for bookmark in bookmarks {
            let has_source = bookmark.tags().contains(source);

            if has_source {
                let mut new_tags = bookmark.tags().clone();
                new_tags.remove(source);
                new_tags.insert(target.clone());

                bookmark.set_tags(new_tags)?;
                count += 1;
            }
        }

        Ok(count)
    }

    fn rename_tag(
        &self,
        bookmarks: &mut [Bookmark],
        old_tag: &Tag,
        new_name: &str,
    ) -> DomainResult<usize> {
        let new_tag = self.create_tag(new_name)?;

        if old_tag == &new_tag {
            return Ok(0); // No change needed
        }

        self.merge_tags(bookmarks, old_tag, &new_tag)
    }

    fn has_tag_changes(&self, bookmark: &Bookmark, new_tags: &HashSet<Tag>) -> bool {
        bookmark.tags() != new_tags
    }

    fn replace_tags(&self, bookmark: &mut Bookmark, tags: HashSet<Tag>) -> DomainResult<()> {
        bookmark.set_tags(tags)
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
            Err(DomainError::TagOperationFailed(format!(
                "Failed to remove tags: {}",
                errors.join(", ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tag(name: &str) -> Tag {
        Tag::new(name).unwrap()
    }

    fn create_test_bookmark(url: &str, tag_names: &[&str]) -> Bookmark {
        let tags: HashSet<Tag> = tag_names
            .iter()
            .map(|&name| create_test_tag(name))
            .collect();

        // Ensure URL is properly formatted
        let valid_url = if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("shell::")
            || url.starts_with("/")
            || url.starts_with("~")
        {
            url.to_string()
        } else {
            format!("https://{}", url)
        };

        Bookmark::new(
            valid_url,
            "Title".to_string(),
            "Description".to_string(),
            tags,
        )
        .unwrap()
    }

    #[test]
    fn test_create_tag() {
        let service = TagServiceImpl::new();

        let tag = service.create_tag("test").unwrap();
        assert_eq!(tag.value(), "test");

        let tag = service.create_tag("TEST").unwrap();
        assert_eq!(tag.value(), "test");

        let result = service.create_tag("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tag_string() {
        let service = TagServiceImpl::new();

        let tags = service.parse_tag_string("tag1,tag2,tag3").unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&create_test_tag("tag1")));
        assert!(tags.contains(&create_test_tag("tag2")));
        assert!(tags.contains(&create_test_tag("tag3")));

        let tags = service.parse_tag_string("tag1, ,tag2").unwrap();
        assert_eq!(tags.len(), 2);

        let tags = service.parse_tag_string("").unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn test_format_tags() {
        let service = TagServiceImpl::new();

        let mut tags = HashSet::new();
        tags.insert(create_test_tag("tag1"));
        tags.insert(create_test_tag("tag2"));

        let formatted = service.format_tags(&tags);
        assert!(formatted == ",tag1,tag2," || formatted == ",tag2,tag1,");

        let tags = HashSet::new();
        let formatted = service.format_tags(&tags);
        assert_eq!(formatted, ",,");
    }

    #[test]
    fn test_is_valid_tag() {
        let service = TagServiceImpl::new();

        assert!(service.is_valid_tag("valid"));
        assert!(service.is_valid_tag("valid-tag"));
        assert!(service.is_valid_tag("123"));

        assert!(!service.is_valid_tag(""));
        assert!(!service.is_valid_tag("invalid,tag"));
        assert!(!service.is_valid_tag("invalid tag"));
    }

    #[test]
    fn test_normalize_tag_name() {
        let service = TagServiceImpl::new();

        assert_eq!(service.normalize_tag_name("Tag"), "tag");
        assert_eq!(service.normalize_tag_name(" tag "), "tag");
        assert_eq!(service.normalize_tag_name("TAG"), "tag");
        assert_eq!(service.normalize_tag_name(""), "");
    }

    #[test]
    fn test_find_all_tags() {
        let service = TagServiceImpl::new();

        let bookmarks = vec![
            create_test_bookmark("url1", &["tag1", "tag2"]),
            create_test_bookmark("url2", &["tag2", "tag3"]),
            create_test_bookmark("url3", &["tag3", "tag4"]),
        ];

        let all_tags = service.find_all_tags(&bookmarks);
        assert_eq!(all_tags.len(), 4);
        assert!(all_tags.contains(&create_test_tag("tag1")));
        assert!(all_tags.contains(&create_test_tag("tag2")));
        assert!(all_tags.contains(&create_test_tag("tag3")));
        assert!(all_tags.contains(&create_test_tag("tag4")));
    }

    #[test]
    fn test_calculate_tag_frequency() {
        let service = TagServiceImpl::new();

        let bookmarks = vec![
            create_test_bookmark("url1", &["tag1", "tag2"]),
            create_test_bookmark("url2", &["tag2", "tag3"]),
            create_test_bookmark("url3", &["tag3", "tag4"]),
        ];

        let frequency = service.calculate_tag_frequency(&bookmarks);
        assert_eq!(frequency.len(), 4);
        assert_eq!(frequency[&create_test_tag("tag1")], 1);
        assert_eq!(frequency[&create_test_tag("tag2")], 2);
        assert_eq!(frequency[&create_test_tag("tag3")], 2);
        assert_eq!(frequency[&create_test_tag("tag4")], 1);
    }

    #[test]
    fn test_find_related_tags() {
        let service = TagServiceImpl::new();

        let bookmarks = vec![
            create_test_bookmark("url1", &["tag1", "tag2", "tag3"]),
            create_test_bookmark("url2", &["tag1", "tag4"]),
            create_test_bookmark("url3", &["tag2", "tag5"]),
        ];

        let related = service.find_related_tags(&create_test_tag("tag1"), &bookmarks);
        assert_eq!(related.len(), 3);
        assert_eq!(related[&create_test_tag("tag2")], 1);
        assert_eq!(related[&create_test_tag("tag3")], 1);
        assert_eq!(related[&create_test_tag("tag4")], 1);
        assert!(!related.contains_key(&create_test_tag("tag5")));
    }

    #[test]
    fn test_merge_tags() {
        let service = TagServiceImpl::new();

        let mut bookmarks = vec![
            create_test_bookmark("url1", &["tag1", "tag2"]),
            create_test_bookmark("url2", &["tag1", "tag3"]),
            create_test_bookmark("url3", &["tag4"]),
        ];

        let source = create_test_tag("tag1");
        let target = create_test_tag("merged");

        let count = service
            .merge_tags(&mut bookmarks, &source, &target)
            .unwrap();
        assert_eq!(count, 2); // Two bookmarks updated

        // Check that tag1 is replaced by merged
        for (i, bookmark) in bookmarks.iter().enumerate() {
            if i < 2 {
                assert!(!bookmark.tags().contains(&source));
                assert!(bookmark.tags().contains(&target));
            }
        }
    }

    #[test]
    fn test_rename_tag() {
        let service = TagServiceImpl::new();

        let mut bookmarks = vec![
            create_test_bookmark("url1", &["tag1", "tag2"]),
            create_test_bookmark("url2", &["tag1", "tag3"]),
        ];

        let old_tag = create_test_tag("tag1");

        let count = service
            .rename_tag(&mut bookmarks, &old_tag, "renamed")
            .unwrap();
        assert_eq!(count, 2); // Two bookmarks updated

        // Check that tag1 is renamed to renamed
        for bookmark in &bookmarks {
            assert!(!bookmark.tags().contains(&old_tag));
            assert!(bookmark.tags().contains(&create_test_tag("renamed")));
        }
    }

    #[test]
    fn test_has_tag_changes() {
        let service = TagServiceImpl::new();

        let bookmark = create_test_bookmark("url", &["tag1", "tag2"]);

        // Same tags, different order
        let mut new_tags = HashSet::new();
        new_tags.insert(create_test_tag("tag2"));
        new_tags.insert(create_test_tag("tag1"));

        assert!(!service.has_tag_changes(&bookmark, &new_tags));

        // Different tags
        let mut new_tags = HashSet::new();
        new_tags.insert(create_test_tag("tag1"));
        new_tags.insert(create_test_tag("tag3"));

        assert!(service.has_tag_changes(&bookmark, &new_tags));
    }
}
