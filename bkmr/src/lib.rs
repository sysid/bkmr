// src/lib.rs
#![crate_type = "lib"]
#![crate_name = "bkmr"]

extern crate skim;

use std::collections::HashSet;

use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::domain::error::DomainResult;
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;

use crate::environment::CONFIG;
use tracing::{debug, error};

// Core modules
pub mod application;
pub mod domain;
pub mod infrastructure;

// CLI modules
pub mod cli;
pub mod context;
pub mod environment;
pub mod util;

/// creates list of normalized tags from "tag1,t2,t3" string
/// be aware of shell parsing rules, so no blanks or quotes
pub fn load_url_details(url: &str) -> DomainResult<(String, String, String)> {
    let client = reqwest::blocking::Client::new();
    let body = client
        .get(url)
        .send()
        .map_err(|e| domain::error::DomainError::CannotFetchMetadata(e.to_string()))?
        .text()
        .map_err(|e| domain::error::DomainError::CannotFetchMetadata(e.to_string()))?;

    let document = select::document::Document::from(body.as_str());

    let title = document
        .find(select::predicate::Name("title"))
        .next()
        .map(|n| n.text().trim().to_owned())
        .unwrap_or_default();

    let description = document
        .find(select::predicate::Attr("name", "description"))
        .next()
        .and_then(|n| n.attr("content"))
        .unwrap_or_default();

    let keywords = document
        .find(select::predicate::Attr("name", "keywords"))
        .next()
        .and_then(|node| node.attr("content"))
        .unwrap_or_default();

    debug!("Keywords {:?}", keywords);

    Ok((title, description.to_owned(), keywords.to_owned()))
}

/// Update bookmarks tags
pub fn update_bookmarks(
    ids: Vec<i32>,
    tags: Vec<String>,
    tags_not: Vec<String>,
    force: bool,
) -> DomainResult<()> {
    let repository = SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| domain::error::DomainError::BookmarkOperationFailed(e.to_string()))?;

    let service = BookmarkApplicationService::new(repository);

    for id in ids {
        update_bm(id, &tags, &tags_not, &service, force).map_err(|e| {
            error!("Error updating bookmark {}: {}", id, e);
            e
        })?;
    }

    Ok(())
}

/// Update a single bookmark's tags
pub fn update_bm(
    id: i32,
    tags: &Vec<String>,
    tags_not: &Vec<String>,
    service: &BookmarkApplicationService<SqliteBookmarkRepository>,
    force: bool,
) -> DomainResult<()> {
    let tags_set: HashSet<String> = tags.iter().cloned().collect();
    let tags_not_set: HashSet<String> = tags_not.iter().cloned().collect();

    // Get the bookmark
    let Some(bookmark) = service
        .get_bookmark(id)
        .map_err(|e| domain::error::DomainError::BookmarkOperationFailed(e.to_string()))?
    else {
        return Err(domain::error::DomainError::BookmarkNotFound(id.to_string()));
    };

    // Process tags
    let new_tags = if force {
        // Just use the provided tags
        tags_set.clone()
    } else {
        // Start with current tags
        let mut result: HashSet<String> = bookmark.tags.into_iter().collect();

        // Add new tags
        result.extend(tags_set);

        // Remove tags_not
        for tag in &tags_not_set {
            result.remove(tag);
        }

        result
    };

    // Convert to Tag domain objects
    let tag_objects = new_tags
        .into_iter()
        .filter_map(|t| Tag::new(&t).ok())
        .collect::<HashSet<_>>();

    // Create update request
    let request = application::dto::BookmarkUpdateRequest {
        id,
        title: None,
        description: None,
        tags: Some(tag_objects.iter().map(|t| t.value().to_string()).collect()),
    };

    // Update the bookmark
    service
        .update_bookmark(request)
        .map_err(|e| domain::error::DomainError::BookmarkOperationFailed(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
/// must be public to be used from integration tests
mod tests {
    use crate::util::testing;
    #[ctor::ctor]
    fn init() {
        testing::init_test_setup().expect("Failed to initialize test setup");
    }
}
