use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::domain::error::DomainResult;  // domain error alias
use crate::domain::tag::Tag;

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

// ------------------ Additional Mapping / Conversions ------------------

impl TagInfoResponse {
    /// Create from a domain `Tag` plus usage count
    pub fn from_domain(tag: &Tag, count: usize) -> Self {
        Self {
            name: tag.value().to_string(),
            count,
        }
    }

    /// Convert a slice of (Tag, frequency) into a vector of responses
    pub fn from_domain_collection(tags_with_counts: &[(Tag, usize)]) -> Vec<Self> {
        tags_with_counts
            .iter()
            .map(|(tag, count)| Self::from_domain(tag, *count))
            .collect()
    }
}

impl TagOperationRequest {
    /// Convert to domain Tag set
    /// If you need this at the domain level, you can return `DomainResult<HashSet<Tag>>`.
    pub fn to_domain_tags(&self) -> DomainResult<HashSet<Tag>> {
        let mut tag_set = HashSet::new();

        for tag_str in &self.tags {
            if tag_str.contains(',') {
                // handle comma-delimited
                let parsed_tags = Tag::parse_tags(tag_str)?;
                tag_set.extend(parsed_tags);
            } else {
                let tag = Tag::new(tag_str)?;
                tag_set.insert(tag);
            }
        }

        Ok(tag_set)
    }

    /// Convert to an internal application DTO if needed
    pub fn to_application_dto(&self) -> crate::application::services::tag::TagOperationDto {
        crate::application::services::tag::TagOperationDto {
            bookmark_ids: self.bookmark_ids.clone(),
            tags: self.tags.clone(),
            replace_existing: self.replace_existing.unwrap_or(false),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;

    #[test]
    fn test_tag_operation_request_single_tag() {
        let req = TagOperationRequest {
            bookmark_ids: vec![1, 2, 3],
            tags: vec!["rust".to_string()],
            replace_existing: Some(true),
        };

        let domain_tags = req.to_domain_tags().unwrap();
        assert_eq!(domain_tags.len(), 1);
        assert!(domain_tags.contains(&Tag::new("rust").unwrap()));
    }

    #[test]
    fn test_tag_operation_request_comma_separated() {
        let req = TagOperationRequest {
            bookmark_ids: vec![42],
            tags: vec!["programming, rust, testing".to_string()],
            replace_existing: None,
        };

        let domain_tags = req.to_domain_tags().unwrap();
        assert_eq!(domain_tags.len(), 3);
        assert!(domain_tags.contains(&Tag::new("programming").unwrap()));
        assert!(domain_tags.contains(&Tag::new("rust").unwrap()));
        assert!(domain_tags.contains(&Tag::new("testing").unwrap()));
    }

    #[test]
    fn test_tag_info_response_from_domain() {
        let tag = Tag::new("example").unwrap();
        let response = TagInfoResponse::from_domain(&tag, 5);

        assert_eq!(response.name, "example");
        assert_eq!(response.count, 5);
    }

    #[test]
    fn test_tag_info_response_from_domain_collection() {
        let t1 = Tag::new("tag1").unwrap();
        let t2 = Tag::new("tag2").unwrap();
        let tags_counts = vec![(t1, 2), (t2, 10)];

        let responses = TagInfoResponse::from_domain_collection(&tags_counts);
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].name, "tag1");
        assert_eq!(responses[0].count, 2);
        assert_eq!(responses[1].name, "tag2");
        assert_eq!(responses[1].count, 10);
    }
}