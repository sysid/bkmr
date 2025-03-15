use serde::{Deserialize, Serialize};
use crate::domain::tag::Tag;
use std::collections::HashSet;

/// Response object for tag information
#[derive(Debug, Clone, Serialize)]
pub struct TagInfoResponse {
    pub name: String,
    pub count: usize,
}

/// Request object for tag operations on bookmarks
#[derive(Debug, Clone, Deserialize)]
pub struct TagOperationRequest {
    pub bookmark_ids: Vec<i32>,
    pub tags: Vec<String>,
    pub replace_existing: Option<bool>,
}

/// Response object for tag suggestions
#[derive(Debug, Clone, Serialize)]
pub struct TagSuggestionResponse {
    pub suggestions: Vec<TagInfoResponse>,
}

/// Request object for merging tags
#[derive(Debug, Clone, Deserialize)]
pub struct TagMergeRequest {
    pub source_tag: String,
    pub target_tag: String,
}

/// Request object for renaming a tag
#[derive(Debug, Clone, Deserialize)]
pub struct TagRenameRequest {
    pub old_name: String,
    pub new_name: String,
}

/// Mapping functions for tag DTOs
impl TagInfoResponse {
    /// Create from domain entity and count
    pub fn from_domain(tag: &Tag, count: usize) -> Self {
        Self {
            name: tag.value().to_string(),
            count,
        }
    }

    /// Create from domain entity collection with counts
    pub fn from_domain_collection(tags_with_counts: &[(Tag, usize)]) -> Vec<Self> {
        tags_with_counts
            .iter()
            .map(|(tag, count)| Self::from_domain(tag, *count))
            .collect()
    }
}

impl TagOperationRequest {
    /// Convert to domain tags
    pub fn to_domain_tags(&self) -> anyhow::Result<HashSet<Tag>> {
        let mut tag_set = HashSet::new();

        for tag_str in &self.tags {
            if tag_str.contains(',') {
                // Handle comma-separated tags
                let parsed_tags = Tag::parse_tags(tag_str)?;
                tag_set.extend(parsed_tags);
            } else {
                // Single tag
                let tag = Tag::new(tag_str)?;
                tag_set.insert(tag);
            }
        }

        Ok(tag_set)
    }

    /// Convert to application DTO
    pub fn to_application_dto(&self) -> crate::application::services::tag::TagOperationDto {
        crate::application::services::tag::TagOperationDto {
            bookmark_ids: self.bookmark_ids.clone(),
            tags: self.tags.clone(),
            replace_existing: self.replace_existing.unwrap_or(false),
        }
    }
}