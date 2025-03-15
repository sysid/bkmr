use std::collections::HashSet;
use anyhow::{Result};

use crate::application::dto::tag_dto::{
    TagInfoResponse, TagMergeRequest, TagOperationRequest,
    TagRenameRequest, TagSuggestionResponse
};
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::domain::services::tag_service::{TagService, TagServiceImpl};
use crate::domain::tag::Tag;

/// Application service for tag operations
pub struct TagApplicationService<R> {
    repository: R,
    domain_service: TagServiceImpl,
}

impl<R> TagApplicationService<R>
where
    R: BookmarkRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            domain_service: TagServiceImpl::new(),
        }
    }

    /// Get all tags with usage count
    pub fn get_all_tags(&self) -> Result<Vec<TagInfoResponse>> {
        let tags = self.repository.get_all_tags()?;

        Ok(tags.into_iter()
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect())
    }

    /// Get tags related to a specific tag
    pub fn get_related_tags(&self, tag_name: &str) -> Result<Vec<TagInfoResponse>> {
        // Create tag domain object
        let tag = self.domain_service.create_tag(tag_name)?;

        // Get related tags
        let related_tags = self.repository.get_related_tags(&tag)?;

        // Convert to DTOs
        Ok(related_tags.into_iter()
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect())
    }

    /// Add tags to bookmarks
    pub fn add_tags_to_bookmarks(&self, request: TagOperationRequest) -> Result<usize> {
        // Parse tags
        let tags = self.parse_tags(&request.tags)?;

        let mut updated_count = 0;

        for id in request.bookmark_ids {
            // Get bookmark
            let mut bookmark = match self.repository.get_by_id(id)? {
                Some(b) => b,
                None => continue, // Skip bookmarks that don't exist
            };

            if request.replace_existing.unwrap_or(false) {
                // Replace all tags
                self.domain_service.replace_tags(&mut bookmark, tags.clone())?;
            } else {
                // Add tags
                self.domain_service.add_tags(&mut bookmark, &tags)?;
            }

            // Save changes
            self.repository.update(&bookmark)?;
            updated_count += 1;
        }

        Ok(updated_count)
    }

    /// Remove tags from bookmarks
    pub fn remove_tags_from_bookmarks(&self, request: TagOperationRequest) -> Result<usize> {
        // Parse tags
        let tags = self.parse_tags(&request.tags)?;

        let mut updated_count = 0;

        for id in request.bookmark_ids {
            // Get bookmark
            let mut bookmark = match self.repository.get_by_id(id)? {
                Some(b) => b,
                None => continue, // Skip bookmarks that don't exist
            };

            // Remove tags
            match self.domain_service.remove_tags(&mut bookmark, &tags) {
                Ok(_) => {
                    // Save changes
                    self.repository.update(&bookmark)?;
                    updated_count += 1;
                },
                Err(_) => continue, // Skip if tags don't exist on bookmark
            }
        }

        Ok(updated_count)
    }

    /// Merge two tags across all bookmarks
    pub fn merge_tags(&self, request: TagMergeRequest) -> Result<usize> {
        // Create tag domain objects
        let source = self.domain_service.create_tag(&request.source_tag)?;
        let target = self.domain_service.create_tag(&request.target_tag)?;

        // Get all bookmarks
        let mut bookmarks = self.repository.get_all()?;

        // Merge tags
        let count = self.domain_service.merge_tags(&mut bookmarks, &source, &target)?;

        // Save changes
        for bookmark in &bookmarks {
            self.repository.update(bookmark)?;
        }

        Ok(count)
    }

    /// Rename a tag across all bookmarks
    pub fn rename_tag(&self, request: TagRenameRequest) -> Result<usize> {
        // Create tag domain object
        let old_tag = self.domain_service.create_tag(&request.old_name)?;

        // Get all bookmarks
        let mut bookmarks = self.repository.get_all()?;

        // Rename tag
        let count = self.domain_service.rename_tag(&mut bookmarks, &old_tag, &request.new_name)?;

        // Save changes
        for bookmark in &bookmarks {
            self.repository.update(bookmark)?;
        }

        Ok(count)
    }

    /// Get tag suggestions based on partial input
    pub fn get_tag_suggestions(&self, partial: &str) -> Result<TagSuggestionResponse> {
        // Get all tags
        let all_tags = self.repository.get_all_tags()?;

        // Filter tags by partial match
        let partial = partial.to_lowercase();
        let filtered_tags = all_tags.into_iter()
            .filter(|(tag, _)| tag.value().to_lowercase().contains(&partial))
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect();

        Ok(TagSuggestionResponse {
            suggestions: filtered_tags,
        })
    }

    /// Parse tag strings into domain Tag objects
    fn parse_tags(&self, tag_strings: &[String]) -> Result<HashSet<Tag>> {
        let mut tags = HashSet::new();

        for tag_str in tag_strings {
            if tag_str.contains(',') {
                // Handle comma-separated tags
                for part in tag_str.split(',') {
                    if !part.trim().is_empty() {
                        tags.insert(self.domain_service.create_tag(part.trim())?);
                    }
                }
            } else {
                // Single tag
                if !tag_str.trim().is_empty() {
                    tags.insert(self.domain_service.create_tag(tag_str)?);
                }
            }
        }

        Ok(tags)
    }
}