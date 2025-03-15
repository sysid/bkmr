use anyhow::{Result};
use std::collections::HashSet;

use crate::application::dto::bookmark_dto::{
    BookmarkCreateRequest, BookmarkListItem, BookmarkResponse, BookmarkSearchRequest,
    BookmarkSearchResponse, BookmarkUpdateRequest,
};
use crate::application::services::search::{SearchParamsDto};
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
    pub fn add_bookmark(&self, request: BookmarkCreateRequest) -> Result<BookmarkResponse> {
        // Check if bookmark already exists
        if self.repository.exists_by_url(&request.url)? {
            return Err(anyhow::anyhow!("Bookmark with this URL already exists"));
        }

        // Extract domain data from request
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
    pub fn update_bookmark(&self, request: BookmarkUpdateRequest) -> Result<BookmarkResponse> {
        // Get existing bookmark
        let bookmark = self
            .repository
            .get_by_id(request.id)?
            .ok_or_else(|| anyhow::anyhow!("Bookmark not found"))?;

        let mut updated_bookmark = bookmark.clone();

        // Update title if provided
        if let Some(ref title) = request.title {
            // Store the current description before mutable borrow
            let current_description = updated_bookmark.description().to_string();

            self.domain_service.update_bookmark_metadata(
                &mut updated_bookmark,
                title,
                &current_description,
            )?;
        }

        // Update description if provided
        if let Some(ref description) = request.description {
            // Store the current title before mutable borrow
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
    pub fn delete_bookmark(&self, id: i32) -> Result<bool> {
        Ok(self.repository.delete(id)?)
    }

    /// Get a bookmark by ID
    pub fn get_bookmark(&self, id: i32) -> Result<Option<BookmarkResponse>> {
        let bookmark = self.repository.get_by_id(id)?;
        Ok(bookmark.map(|b| BookmarkResponse::from_domain(&b)))
    }

    /// Search bookmarks with query params
    pub fn search_bookmarks(
        &self,
        request: BookmarkSearchRequest,
    ) -> Result<BookmarkSearchResponse> {
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

        // Build query specification
        let mut query = BookmarkQuery::new();

        // Add text search if provided
        if let Some(text) = params.query {
            if !text.is_empty() {
                query = query.with_specification(TextSearchSpecification::new(text));
            }
        }

        // Add tag specifications
        // These could be combined more elegantly, but we're keeping it explicit for clarity
        let mut compound_spec = None;

        // All tags
        if let Some(tags) = params.all_tags {
            let tag_set = self.parse_tags(tags)?;
            let spec = AllTagsSpecification::new(tag_set);
            compound_spec = Some(Box::new(spec) as Box<dyn Specification<Bookmark>>);
        }

        // Any tags
        if let Some(tags) = params.any_tags {
            let tag_set = self.parse_tags(tags)?;
            let spec = AnyTagSpecification::new(tag_set);

            if let Some(existing_spec) = compound_spec {
                compound_spec = Some(Box::new(
                    crate::domain::repositories::query::AndSpecification::new(existing_spec, spec),
                ));
            } else {
                compound_spec = Some(Box::new(spec));
            }
        }

        // Exclude all tags
        if let Some(tags) = params.exclude_all_tags {
            let tag_set = self.parse_tags(tags)?;
            let spec = NotSpecification::new(AllTagsSpecification::new(tag_set));

            if let Some(existing_spec) = compound_spec {
                compound_spec = Some(Box::new(
                    crate::domain::repositories::query::AndSpecification::new(existing_spec, spec),
                ));
            } else {
                compound_spec = Some(Box::new(spec));
            }
        }

        // Exclude any tags
        if let Some(tags) = params.exclude_any_tags {
            let tag_set = self.parse_tags(tags)?;
            let spec = NotSpecification::new(AnyTagSpecification::new(tag_set));

            if let Some(existing_spec) = compound_spec {
                compound_spec = Some(Box::new(
                    crate::domain::repositories::query::AndSpecification::new(existing_spec, spec),
                ));
            } else {
                compound_spec = Some(Box::new(spec));
            }
        }

        // Exact tags
        if let Some(tags) = params.exact_tags {
            let tag_set = self.parse_tags(tags)?;
            let spec = ExactTagsSpecification::new(tag_set);

            // Exact tags override other tag specifications
            compound_spec = Some(Box::new(spec));
        }

        // Add the compound specification if any
        if let Some(spec) = compound_spec {
            query = query.with_specification_boxed(spec);
        }

        // Add sorting
        if params.sort_by_date.unwrap_or(false) {
            let direction = if params.sort_descending.unwrap_or(true) {
                SortDirection::Descending
            } else {
                SortDirection::Ascending
            };
            query = query.with_sort_by_date(direction);
        }

        // Add pagination
        if let Some(limit) = params.limit {
            query = query.with_limit(limit);
        }

        if let Some(offset) = params.offset {
            query = query.with_offset(offset);
        }

        // Execute query
        let bookmarks = self.repository.search(&query)?;
        let total_count = bookmarks.len(); // In a real app, we'd get this from the repository

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
    pub fn record_bookmark_access(&self, id: i32) -> Result<()> {
        // Get bookmark
        let mut bookmark = self
            .repository
            .get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Bookmark not found"))?;

        // Update access count
        self.domain_service.record_access(&mut bookmark)?;

        // Save changes
        self.repository.update(&bookmark)?;

        Ok(())
    }

    /// Fetch metadata for a URL
    pub fn fetch_url_metadata(&self, url: &str) -> Result<(String, String, String)> {
        self.domain_service.fetch_metadata(url)
    }

    /// Helper method to parse tags from string vector
    fn parse_tags(&self, tag_strings: Vec<String>) -> DomainResult<HashSet<Tag>> {
        let mut tags = HashSet::new();

        for tag_str in tag_strings {
            // Skip empty strings
            if tag_str.trim().is_empty() {
                continue;
            }

            // Parse tag string (might contain multiple comma-separated tags)
            let mut parsed_tags = Tag::parse_tags(&tag_str)?;
            tags.extend(parsed_tags.drain());
        }

        Ok(tags)
    }
}
