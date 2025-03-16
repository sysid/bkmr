use std::collections::HashSet;

use crate::application::dto::bookmark_dto::{
    BookmarkCreateRequest, BookmarkListItem, BookmarkResponse, BookmarkSearchRequest,
    BookmarkSearchResponse, BookmarkUpdateRequest,
};
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::services::search::SearchParamsDto;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::domain::repositories::query::{
    AllTagsSpecification, AnyTagSpecification, BookmarkQuery, ExactTagsSpecification,
    NotSpecification, SortDirection, Specification, TextSearchSpecification,
};
use crate::domain::services::bookmark_service::{BookmarkService, BookmarkServiceImpl};
use crate::domain::tag::Tag;

/// Application service for bookmark operations
pub struct BookmarkApplicationService<R> {
    pub(crate) repository: R,
    domain_service: BookmarkServiceImpl,
}

impl<R> BookmarkApplicationService<R>
where
    R: BookmarkRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            domain_service: BookmarkServiceImpl::new(),
        }
    }

    /// Add a new bookmark
    pub fn add_bookmark(
        &self,
        request: BookmarkCreateRequest,
    ) -> ApplicationResult<BookmarkResponse> {
        // Check if bookmark already exists
        if self.repository.exists_by_url(&request.url)? {
            return Err(ApplicationError::BookmarkExists(request.url.clone()));
        }

        // Convert incoming DTO to domain objects (URL, title, description, tags).
        // If there's a domain-level parsing failure, it becomes ApplicationError::Domain automatically.
        let (url, title, description, tags) = request.to_domain_objects()?;

        // Create bookmark domain entity
        let mut bookmark = self
            .domain_service
            .create_bookmark(&url, &title, &description, tags)?;

        // Persist bookmark
        self.repository.add(&mut bookmark)?;

        // Return DTO
        Ok(BookmarkResponse::from_domain(&bookmark))
    }

    /// Update an existing bookmark
    pub fn update_bookmark(
        &self,
        request: BookmarkUpdateRequest,
    ) -> ApplicationResult<BookmarkResponse> {
        // Get existing bookmark (or return a typed not-found error)
        let bookmark = self
            .repository
            .get_by_id(request.id)?
            .ok_or(ApplicationError::BookmarkNotFound(request.id))?;

        let mut updated_bookmark = bookmark.clone();

        // Update title if provided
        if let Some(ref title) = request.title {
            let current_description = updated_bookmark.description().to_string();

            self.domain_service.update_bookmark_metadata(
                &mut updated_bookmark,
                title,
                &current_description,
            )?;
        }

        // Update description if provided
        if let Some(ref description) = request.description {
            let current_title = updated_bookmark.title().to_string();

            self.domain_service.update_bookmark_metadata(
                &mut updated_bookmark,
                &current_title,
                description,
            )?;
        }

        // Update tags if provided
        if let Some(tags) = request.to_domain_tags()? {
            self.domain_service
                .replace_tags(&mut updated_bookmark, tags)?;
        }

        // Persist changes
        self.repository.update(&updated_bookmark)?;

        // Return updated DTO
        Ok(BookmarkResponse::from_domain(&updated_bookmark))
    }

    /// Delete a bookmark
    pub fn delete_bookmark(&self, id: i32) -> ApplicationResult<bool> {
        // The repository returns Ok(false) if no bookmark is deleted
        let deleted = self.repository.delete(id)?;
        Ok(deleted)
    }

    /// Get a bookmark by ID
    pub fn get_bookmark(&self, id: i32) -> ApplicationResult<Option<BookmarkResponse>> {
        let bookmark = self.repository.get_by_id(id)?;
        Ok(bookmark.map(|b| BookmarkResponse::from_domain(&b)))
    }

    /// Search bookmarks with query params
    pub fn search_bookmarks(
        &self,
        request: BookmarkSearchRequest,
    ) -> ApplicationResult<BookmarkSearchResponse> {
        // Convert to search params
        let params = SearchParamsDto {
            query: request.query.clone(),
            all_tags: request.all_tags.clone(),
            any_tags: request.any_tags.clone(),
            exclude_all_tags: request.exclude_all_tags.clone(),
            exclude_any_tags: request.exclude_any_tags.clone(),
            exact_tags: request.exact_tags.clone(),
            sort_by_date: request.sort_by_date,
            sort_descending: request.sort_descending,
            limit: request.limit,
            offset: request.offset,
        };

        // Build query
        let mut query = BookmarkQuery::new();

        // 1) Text search
        if let Some(text) = &params.query {
            if !text.is_empty() {
                query = query.with_specification(TextSearchSpecification::new(text.clone()));
            }
        }

        // 2) Tag specifications (all_tags, any_tags, exclude, etc.)
        let mut compound_spec: Option<Box<dyn Specification<Bookmark>>> = None;

        // All tags
        if let Some(tags) = &params.all_tags {
            let spec = AllTagsSpecification::new(self.parse_tags(tags.clone())?);
            compound_spec = Some(Box::new(spec));
        }

        // Any tags
        if let Some(tags) = &params.any_tags {
            let spec = AnyTagSpecification::new(self.parse_tags(tags.clone())?);
            compound_spec = Self::chain_specs(compound_spec, spec);
        }

        // Exclude all
        if let Some(tags) = &params.exclude_all_tags {
            let spec =
                NotSpecification::new(AllTagsSpecification::new(self.parse_tags(tags.clone())?));
            compound_spec = Self::chain_specs(compound_spec, spec);
        }

        // Exclude any
        if let Some(tags) = &params.exclude_any_tags {
            let spec =
                NotSpecification::new(AnyTagSpecification::new(self.parse_tags(tags.clone())?));
            compound_spec = Self::chain_specs(compound_spec, spec);
        }

        // Exact
        if let Some(tags) = &params.exact_tags {
            let spec = ExactTagsSpecification::new(self.parse_tags(tags.clone())?);
            // If user requests EXACT, we override earlier tag specs
            compound_spec = Some(Box::new(spec));
        }

        // If we ended up with a compound spec, attach it
        if let Some(final_spec) = compound_spec {
            query = query.with_specification_boxed(final_spec);
        }

        // 3) Sorting
        if params.sort_by_date.unwrap_or(false) {
            let direction = if params.sort_descending.unwrap_or(true) {
                SortDirection::Descending
            } else {
                SortDirection::Ascending
            };
            query = query.with_sort_by_date(direction);
        }

        // 4) Pagination
        if let Some(limit) = params.limit {
            query = query.with_limit(limit);
        }

        if let Some(offset) = params.offset {
            query = query.with_offset(offset);
        }

        // Execute
        let bookmarks = self.repository.search(&query)?;
        let total_count = bookmarks.len(); // in-memory count

        // Convert to response
        Ok(BookmarkSearchResponse {
            bookmarks: BookmarkListItem::from_domain_collection(&bookmarks),
            total_count,
            page: params
                .offset
                .map(|offset| offset / params.limit.unwrap_or(10) + 1),
            page_size: params.limit,
            has_more: total_count
                > (params.offset.unwrap_or(0) + params.limit.unwrap_or(total_count)),
        })
    }

    /// Record that a bookmark was accessed
    pub fn record_bookmark_access(&self, id: i32) -> ApplicationResult<()> {
        // Find bookmark or return typed NotFound
        let mut bookmark = self
            .repository
            .get_by_id(id)?
            .ok_or(ApplicationError::BookmarkNotFound(id))?;

        // Update domain logic
        self.domain_service.record_access(&mut bookmark)?;

        // Persist
        self.repository.update(&bookmark)?;
        Ok(())
    }

    /// Fetch metadata for a URL (domain-level operation)
    pub fn fetch_url_metadata(&self, url: &str) -> ApplicationResult<(String, String, String)> {
        // If domain logic fails, it becomes ApplicationError::Domain
        let meta = self.domain_service.fetch_metadata(url)?;
        Ok(meta)
    }

    /// Helper to parse tags from a list of strings
    fn parse_tags(&self, tag_strings: Vec<String>) -> DomainResult<HashSet<Tag>> {
        let mut tags = HashSet::new();

        for tag_str in tag_strings {
            if tag_str.trim().is_empty() {
                continue;
            }
            // Tag::parse_tags returns DomainError
            let mut parsed = Tag::parse_tags(&tag_str)?;
            tags.extend(parsed.drain());
        }
        Ok(tags)
    }

    /// Helper to chain tag specs with AND logic
    fn chain_specs<S: Specification<Bookmark> + 'static>(
        existing_spec: Option<Box<dyn Specification<Bookmark>>>,
        new_spec: S,
    ) -> Option<Box<dyn Specification<Bookmark>>> {
        match existing_spec {
            Some(e) => Some(Box::new(
                crate::domain::repositories::query::AndSpecification::new(e, new_spec),
            )),
            None => Some(Box::new(new_spec)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    // 1) Pull in the sqlite repo (instead of the in-memory one)
    use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;

    // For creating temporary SQLite files
    use tempfile::NamedTempFile;

    // Helper function to create a fresh SQLite test repo + service.
    fn create_service_and_repo() -> (
        BookmarkApplicationService<SqliteBookmarkRepository>,
        SqliteBookmarkRepository,
        NamedTempFile,
    ) {
        // 2) Create a temporary file on disk
        let tmpfile = NamedTempFile::new().expect("Failed to create temp file for SQLite");
        let db_path = tmpfile.path().to_str().unwrap().to_string();

        // 3) Build the SQLite repo from that path
        let repo = SqliteBookmarkRepository::from_url(&db_path)
            .expect("Failed to initialize SqliteBookmarkRepository");

        // 4) Construct the application service
        let service = BookmarkApplicationService::new(repo.clone());

        // 5) Return (service, repo, the temp file handle)
        (service, repo, tmpfile)
    }

    #[test]
    fn test_add_bookmark_success() {
        // This now returns (service, repo, _tmpfile),
        // but we only need the service for this test.
        let (service, _repo, _tmpfile) = create_service_and_repo();

        let request = BookmarkCreateRequest {
            url: "https://example.com".into(),
            title: Some("Example".into()),
            description: Some("A test".into()),
            tags: Some(vec!["rust".into()]),
            fetch_metadata: None,
        };

        let response = service.add_bookmark(request).unwrap();
        assert_eq!(response.url, "https://example.com");
        assert_eq!(response.title, "Example");
        assert_eq!(response.description, "A test");
        assert!(response.tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_add_bookmark_already_exists() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Insert a bookmark with the same URL
        let mut existing =
            Bookmark::new("https://example.com", "Existing", "Desc", HashSet::new()).unwrap();
        repo.add(&mut existing).unwrap();

        let request = BookmarkCreateRequest {
            url: "https://example.com".into(),
            title: Some("Duplicate".into()),
            description: None,
            tags: None,
            fetch_metadata: None,
        };

        let err = service.add_bookmark(request).unwrap_err();
        match err {
            ApplicationError::BookmarkExists(url) => {
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("Expected BookmarkExists error"),
        }
    }

    #[test]
    fn test_update_bookmark_success() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Insert a bookmark
        let mut bookmark = Bookmark::new(
            "https://example.com",
            "Old Title",
            "Old Desc",
            HashSet::new(),
        )
        .unwrap();
        repo.add(&mut bookmark).unwrap();
        let id = bookmark.id().unwrap();

        // Prepare an update
        let request = BookmarkUpdateRequest {
            id,
            title: Some("New Title".into()),
            description: Some("New Description".into()),
            tags: Some(vec!["rust".into()]),
        };

        let response = service.update_bookmark(request).unwrap();
        assert_eq!(response.title, "New Title");
        assert_eq!(response.description, "New Description");
        assert_eq!(response.tags.len(), 1);
        assert!(response.tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_update_bookmark_not_found() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        let request = BookmarkUpdateRequest {
            id: 999,
            title: Some("Not Found".into()),
            description: None,
            tags: None,
        };

        let err = service.update_bookmark(request).unwrap_err();
        match err {
            ApplicationError::BookmarkNotFound(id) => assert_eq!(id, 999),
            _ => panic!("Expected BookmarkNotFound error"),
        }
    }

    #[test]
    fn test_delete_bookmark() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Insert a bookmark
        let mut bookmark =
            Bookmark::new("https://example.com", "Delete Me", "Desc", HashSet::new()).unwrap();
        repo.add(&mut bookmark).unwrap();
        let id = bookmark.id().unwrap();

        // Delete it
        let deleted = service.delete_bookmark(id).unwrap();
        assert!(deleted);

        // Trying again returns false
        let deleted_again = service.delete_bookmark(id).unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_get_bookmark() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        let mut b =
            Bookmark::new("https://example.com", "Some Title", "Desc", HashSet::new()).unwrap();
        repo.add(&mut b).unwrap();
        let id = b.id().unwrap();

        let found = service.get_bookmark(id).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().url, "https://example.com");

        let not_found = service.get_bookmark(999).unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_search_bookmarks() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Insert multiple bookmarks
        let mut b1 = Bookmark::new(
            "https://rust-lang.org",
            "Rust Lang",
            "Rust language",
            HashSet::new(),
        )
        .unwrap();
        let mut b2 = Bookmark::new(
            "https://python.org",
            "Python Lang",
            "Python language",
            HashSet::new(),
        )
        .unwrap();
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        // Make a search request
        let request = BookmarkSearchRequest {
            query: Some("rust".into()),
            ..Default::default()
        };

        let response = service.search_bookmarks(request).unwrap();
        assert_eq!(response.total_count, 1);
        assert_eq!(response.bookmarks[0].url, "https://rust-lang.org");
    }

    #[test]
    fn test_record_bookmark_access() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        let mut b = Bookmark::new("https://example.com", "Title", "Desc", HashSet::new()).unwrap();
        repo.add(&mut b).unwrap();
        let id = b.id().unwrap();

        service.record_bookmark_access(id).unwrap();
        service.record_bookmark_access(id).unwrap();

        let updated = repo.get_by_id(id).unwrap().unwrap();
        assert_eq!(updated.access_count(), 2);
    }

    #[test]
    fn test_fetch_url_metadata() {
        let (service, repo, _tmpfile) = create_service_and_repo();

        // Currently returns (String::new(), String::new(), String::new())
        let (title, desc, extra) = service.fetch_url_metadata("https://example.com").unwrap();
        assert_eq!(title, "");
        assert_eq!(desc, "");
        assert_eq!(extra, "");
    }
}
