use std::collections::HashSet;

use crate::application::dto::tag_dto::{
    TagInfoResponse, TagMergeRequest, TagOperationRequest, TagRenameRequest, TagSuggestionResponse
};
use crate::application::error::{ApplicationError, ApplicationResult};
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
    pub fn get_all_tags(&self) -> ApplicationResult<Vec<TagInfoResponse>> {
        let tags_with_counts = self.repository.get_all_tags()?; // DomainError => ApplicationError::Domain
        let responses = tags_with_counts
            .into_iter()
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect();
        Ok(responses)
    }

    /// Get tags related to a specific tag
    pub fn get_related_tags(&self, tag_name: &str) -> ApplicationResult<Vec<TagInfoResponse>> {
        // Create the domain object
        let tag = self.domain_service.create_tag(tag_name)?; // DomainError => ApplicationError::Domain

        // Get related tags from repository
        let related_tags = self.repository.get_related_tags(&tag)?;

        // Convert to DTO
        let responses = related_tags
            .into_iter()
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect();
        Ok(responses)
    }

    /// Add tags to bookmarks
    pub fn add_tags_to_bookmarks(&self, request: TagOperationRequest) -> ApplicationResult<usize> {
        let tags = self.parse_tags(&request.tags)?;

        let mut updated_count = 0;
        for id in &request.bookmark_ids {
            let mut bookmark = match self.repository.get_by_id(*id)? {
                Some(b) => b,
                None => continue, // Skip if bookmark doesn't exist
            };

            if request.replace_existing.unwrap_or(false) {
                // Replace all tags
                self.domain_service.replace_tags(&mut bookmark, tags.clone())?;
            } else {
                // Add tags
                self.domain_service.add_tags(&mut bookmark, &tags)?;
            }

            // Persist changes
            self.repository.update(&bookmark)?;
            updated_count += 1;
        }

        Ok(updated_count)
    }

    /// Remove tags from bookmarks
    pub fn remove_tags_from_bookmarks(&self, request: TagOperationRequest) -> ApplicationResult<usize> {
        let tags = self.parse_tags(&request.tags)?;

        let mut updated_count = 0;
        for id in &request.bookmark_ids {
            let mut bookmark = match self.repository.get_by_id(*id)? {
                Some(b) => b,
                None => continue,
            };

            // If remove_tags fails, we skip that bookmark
            if self.domain_service.remove_tags(&mut bookmark, &tags).is_ok() {
                self.repository.update(&bookmark)?;
                updated_count += 1;
            }
        }

        Ok(updated_count)
    }

    /// Merge two tags across all bookmarks
    pub fn merge_tags(&self, request: TagMergeRequest) -> ApplicationResult<usize> {
        let source = self.domain_service.create_tag(&request.source_tag)?;
        let target = self.domain_service.create_tag(&request.target_tag)?;

        let mut bookmarks = self.repository.get_all()?;
        let count = self.domain_service.merge_tags(&mut bookmarks, &source, &target)?;

        // Save all updated bookmarks
        for bookmark in &bookmarks {
            self.repository.update(bookmark)?;
        }

        Ok(count)
    }

    /// Rename a tag across all bookmarks
    pub fn rename_tag(&self, request: TagRenameRequest) -> ApplicationResult<usize> {
        let old_tag = self.domain_service.create_tag(&request.old_name)?;

        let mut bookmarks = self.repository.get_all()?;
        let count = self.domain_service.rename_tag(&mut bookmarks, &old_tag, &request.new_name)?;

        // Save all updated
        for bookmark in &bookmarks {
            self.repository.update(bookmark)?;
        }

        Ok(count)
    }

    /// Get tag suggestions based on partial input
    pub fn get_tag_suggestions(&self, partial: &str) -> ApplicationResult<TagSuggestionResponse> {
        let all_tags = self.repository.get_all_tags()?;
        let partial_lower = partial.to_lowercase();

        // Filter tags that contain the partial string
        let suggestions = all_tags
            .into_iter()
            .filter(|(tag, _)| tag.value().to_lowercase().contains(&partial_lower))
            .map(|(tag, count)| TagInfoResponse {
                name: tag.value().to_string(),
                count,
            })
            .collect();

        Ok(TagSuggestionResponse { suggestions })
    }

    /// Parse a vector of raw tag strings into domain Tag objects
    fn parse_tags(&self, tag_strings: &[String]) -> ApplicationResult<HashSet<Tag>> {
        let mut tags = HashSet::new();
        for tag_str in tag_strings {
            // Possibly split by comma
            if tag_str.contains(',') {
                for part in tag_str.split(',') {
                    let part = part.trim();
                    if !part.is_empty() {
                        let tag = self.domain_service.create_tag(part)?; // DomainError => ApplicationError::Domain
                        tags.insert(tag);
                    }
                }
            } else {
                let trimmed = tag_str.trim();
                if !trimmed.is_empty() {
                    let tag = self.domain_service.create_tag(trimmed)?;
                    tags.insert(tag);
                }
            }
        }
        Ok(tags)
    }
}
