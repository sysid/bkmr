use std::collections::HashSet;

use crate::application::dto::tag_dto::{
    TagInfoResponse, TagMergeRequest, TagOperationRequest, TagRenameRequest, TagSuggestionResponse,
};
use crate::application::error::ApplicationResult;
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
                self.domain_service
                    .replace_tags(&mut bookmark, tags.clone())?;
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
    pub fn remove_tags_from_bookmarks(
        &self,
        request: TagOperationRequest,
    ) -> ApplicationResult<usize> {
        let tags = self.parse_tags(&request.tags)?;

        let mut updated_count = 0;
        for id in &request.bookmark_ids {
            let mut bookmark = match self.repository.get_by_id(*id)? {
                Some(b) => b,
                None => continue,
            };

            // If remove_tags fails, we skip that bookmark
            if self
                .domain_service
                .remove_tags(&mut bookmark, &tags)
                .is_ok()
            {
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
        let count = self
            .domain_service
            .merge_tags(&mut bookmarks, &source, &target)?;

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
        let count = self
            .domain_service
            .rename_tag(&mut bookmarks, &old_tag, &request.new_name)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
    use maplit::hashset;
    use serial_test::serial;
    use std::collections::HashSet;
    use tracing::Instrument;

    // Returns a stable path in `target/test_db/`.
    // We recreate (or remove) the DB file each time for a clean slate,
    // but we do NOT delete it after tests, so you can inspect it later.
    fn create_persistent_db_path() -> String {
        // e.g. put in `target/test_db/bookmarks_test.sqlite`
        let db_dir = std::env::current_dir()
            .expect("Failed to get current dir")
            .join("target")
            .join("test_db");
        std::fs::create_dir_all(&db_dir).expect("Failed to create test_db directory");

        let db_path = db_dir.join("bookmarks_test.sqlite");

        // If the file already exists, remove it so tests start with a clean DB
        if db_path.exists() {
            std::fs::remove_file(&db_path).expect("Failed to remove old test DB file");
        }

        db_path.to_str().expect("Non-UTF8 path?").to_string()
    }

    // Helper function to create a fresh SQLite test repo + service.
    fn create_service_and_repo() -> (
        TagApplicationService<SqliteBookmarkRepository>,
        SqliteBookmarkRepository,
        String,
    ) {
        // 2) Create a temporary file on disk
        let db_path = create_persistent_db_path();

        // 3) Build the SQLite repo from that path
        let repo = SqliteBookmarkRepository::from_url(&db_path)
            .expect("Failed to initialize SqliteBookmarkRepository");

        // 4) Construct the application service
        let service = TagApplicationService::new(repo.clone());

        // 5) Return (service, repo, the temp file handle)
        (service, repo, db_path)
    }

    #[test]
    #[serial]
    fn test_get_all_tags() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Add bookmarks with tags
        let mut b1 = Bookmark::new(
            "https://example.com",
            "Title1",
            "Desc1",
            hashset! { Tag::new("rust").unwrap(), Tag::new("lang").unwrap() },
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://another.com",
            "Title2",
            "Desc2",
            hashset! { Tag::new("lang").unwrap(), Tag::new("python").unwrap() },
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        let tags = service.get_all_tags().unwrap();
        // For example: rust -> 1, lang -> 2, python -> 1
        assert_eq!(tags.len(), 8); // 3 tags + 5 pre-loaded tags
    }

    #[test]
    #[serial]
    fn test_get_related_tags() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Add bookmarks
        let mut b1 = Bookmark::new(
            "https://example.com",
            "Test",
            "Desc",
            hashset! { Tag::new("rust").unwrap(), Tag::new("lang").unwrap() },
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://another.com",
            "Test2",
            "Desc2",
            hashset! { Tag::new("rust").unwrap(), Tag::new("web").unwrap() },
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        // Check related to 'rust'
        let related = service.get_related_tags("rust").unwrap();
        // Might contain 'lang' -> 1, 'web' -> 1
        assert_eq!(related.len(), 2);
    }

    #[test]
    #[serial]
    fn test_add_tags_to_bookmarks() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Insert a bookmark
        let mut b = Bookmark::new("https://example.com", "Title", "Desc", HashSet::new()).unwrap();
        repo.add(&mut b).unwrap();
        let id = b.id().unwrap();

        let request = TagOperationRequest {
            bookmark_ids: vec![id],
            tags: vec!["rust".into(), "programming".into()],
            replace_existing: Some(false),
        };

        let updated_count = service.add_tags_to_bookmarks(request).unwrap();
        assert_eq!(updated_count, 1);

        let updated = repo.get_by_id(id).unwrap().unwrap();
        let updated_tags = updated.tags();
        assert_eq!(updated_tags.len(), 2);
        assert!(updated_tags.contains(&Tag::new("rust").unwrap()));
        assert!(updated_tags.contains(&Tag::new("programming").unwrap()));
    }

    #[test]
    #[serial]
    fn test_remove_tags_from_bookmarks() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        let mut b = Bookmark::new(
            "https://example.com",
            "Title",
            "Desc",
            hashset! { Tag::new("rust").unwrap(), Tag::new("lang").unwrap() },
        )
        .unwrap();
        repo.add(&mut b).unwrap();
        let id = b.id().unwrap();

        let request = TagOperationRequest {
            bookmark_ids: vec![id],
            tags: vec!["rust".into()],
            replace_existing: None,
        };

        let updated_count = service.remove_tags_from_bookmarks(request).unwrap();
        assert_eq!(updated_count, 1);

        let updated = repo.get_by_id(id).unwrap().unwrap();
        assert_eq!(updated.tags().len(), 1);
        assert!(updated.tags().contains(&Tag::new("lang").unwrap()));
        assert!(!updated.tags().contains(&Tag::new("rust").unwrap()));
    }

    #[test]
    #[serial]
    fn test_merge_tags() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Two bookmarks each containing source tag
        let mut b1 = Bookmark::new(
            "https://example1.com",
            "Title1",
            "Desc1",
            hashset! { Tag::new("source").unwrap() },
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://example2.com",
            "Title2",
            "Desc2",
            hashset! { Tag::new("source").unwrap(), Tag::new("other").unwrap() },
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        let request = TagMergeRequest {
            source_tag: "source".into(),
            target_tag: "target".into(),
        };

        let updated_count = service.merge_tags(request).unwrap();
        assert_eq!(updated_count, 2);

        // Check that 'source' replaced with 'target'
        let all = repo.get_all().unwrap();
        let any_source_exists = all
            .iter()
            .flat_map(|bm| bm.tags().iter()) // Flatten all tags into an iterator
            .any(|tag| tag.value() == "source"); // Check if "source" exists

        assert!(!any_source_exists, "'source' tag should not be present");
    }

    #[test]
    #[serial]
    fn test_rename_tag() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Bookmarks with old tag
        let mut b1 = Bookmark::new(
            "https://example1.com",
            "Title1",
            "Desc1",
            hashset! { Tag::new("oldtag").unwrap() },
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://example2.com",
            "Title2",
            "Desc2",
            hashset! { Tag::new("oldtag").unwrap(), Tag::new("other").unwrap() },
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        let request = TagRenameRequest {
            old_name: "oldtag".into(),
            new_name: "newtag".into(),
        };

        let updated_count = service.rename_tag(request).unwrap();
        assert_eq!(updated_count, 2);

        let all = repo.get_all().unwrap();
        let any_source_exists = all
            .iter()
            .flat_map(|bm| bm.tags().iter()) // Flatten all tags into an iterator
            .any(|tag| tag.value() == "oldtag"); // Check if "source" exists

        assert!(!any_source_exists, "'oldtag' tag should not be present");
    }

    #[test]
    #[serial]
    fn test_get_tag_suggestions() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Bookmarks with tags
        let mut b1 = Bookmark::new(
            "https://example1.com",
            "Title1",
            "Desc1",
            hashset! { Tag::new("rust").unwrap(), Tag::new("lang").unwrap() },
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://example2.com",
            "Title2",
            "Desc2",
            hashset! { Tag::new("rubble").unwrap() },
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        // Query partial 'ru'
        let suggestions = service.get_tag_suggestions("ru").unwrap();
        // Might contain 'rust', 'rubble'
        assert_eq!(suggestions.suggestions.len(), 2);
        let found_names: Vec<_> = suggestions.suggestions.iter().map(|s| &s.name).collect();
        assert!(found_names.contains(&&"rust".to_string()));
        assert!(found_names.contains(&&"rubble".to_string()));
    }
}
