// src/application/services/tag_service.rs
use crate::application::error::ApplicationResult;
use crate::domain::tag::Tag;

/// Service interface for tag-related operations
pub trait TagService: Send + Sync {
    /// Get all tags with their usage counts
    fn get_all_tags(&self) -> ApplicationResult<Vec<(Tag, usize)>>;

    /// Get tags related to the given tag
    fn get_related_tags(&self, tag: &Tag) -> ApplicationResult<Vec<(Tag, usize)>>;

    /// Parse a tag string and create Tag objects
    fn parse_tag_string(&self, tag_str: &str) -> ApplicationResult<Vec<Tag>>;
}
