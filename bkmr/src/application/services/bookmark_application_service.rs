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
    repository: R,
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
    pub fn add_bookmark(&self, request: BookmarkCreateRequest) -> ApplicationResult<BookmarkResponse> {
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
    pub fn update_bookmark(&self, request: BookmarkUpdateRequest) -> ApplicationResult<BookmarkResponse> {
        // Get existing bookmark (or return a typed not-found error)
        let bookmark = self
            .repository
            .get_by_id(request.id)?
            .ok_or_else(|| ApplicationError::BookmarkNotFound(request.id))?;

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
            self.domain_service.replace_tags(&mut updated_bookmark, tags)?;
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
            let spec = NotSpecification::new(AllTagsSpecification::new(self.parse_tags(tags.clone())?));
            compound_spec = Self::chain_specs(compound_spec, spec);
        }

        // Exclude any
        if let Some(tags) = &params.exclude_any_tags {
            let spec = NotSpecification::new(AnyTagSpecification::new(self.parse_tags(tags.clone())?));
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
            .ok_or_else(|| ApplicationError::BookmarkNotFound(id))?;

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
            Some(e) => Some(Box::new(crate::domain::repositories::query::AndSpecification::new(e, new_spec))),
            None => Some(Box::new(new_spec)),
        }
    }
}
