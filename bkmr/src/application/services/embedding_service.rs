// src/application/services/embedding_service.rs

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::context::Context;
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::infrastructure::embeddings::serialize_embedding;
use tracing::{debug, info};

pub struct EmbeddingService<R> {
    repository: R,
}

impl<R> EmbeddingService<R>
where
    R: BookmarkRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Backfill embeddings for bookmarks without them
    pub fn backfill_embeddings(&self, dry_run: bool) -> ApplicationResult<usize> {
        let bookmarks = self.repository.get_without_embeddings()?;
        info!("Found {} bookmarks without embeddings", bookmarks.len());

        if dry_run {
            for bookmark in &bookmarks {
                debug!("Would generate embedding for: {}", bookmark.title());
            }
            return Ok(bookmarks.len());
        }

        let mut updated_count = 0;
        for bookmark in bookmarks {
            debug!("Generating embedding for: {}", bookmark.title());

            // Generate content for embedding
            let content = bookmark.get_content_for_embedding();

            // Generate embedding
            if let Some(embedding) = Context::read_global().get_embedding(&content) {
                // Create a mutable copy for updating
                let mut bookmark_to_update = bookmark.clone();

                // Update the bookmark
                self.repository.update(&bookmark_to_update)?;
                updated_count += 1;
                info!(
                    "Updated embedding for bookmark ID {}",
                    bookmark_to_update.id().unwrap_or(0)
                );
            }
        }

        info!("Updated embeddings for {} bookmarks", updated_count);
        Ok(updated_count)
    }

    pub fn get_bookmark_embedding(&self, bookmark_id: i32) -> ApplicationResult<Option<Vec<u8>>> {
        let bookmark = self
            .repository
            .get_by_id(bookmark_id)?
            .ok_or_else(|| ApplicationError::BookmarkNotFound(bookmark_id))?;

        // Get content for embedding
        let content = bookmark.get_content_for_embedding();

        // Get embedding from context
        let embedding = Context::read_global().get_embedding(&content);

        Ok(embedding)
    }

    /// Generate content for embedding from a bookmark
    fn generate_embedding_content(&self, bookmark: &Bookmark) -> String {
        bookmark.get_content_for_embedding()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::Mutex;

    // Mock repository for testing
    struct MockBookmarkRepository {
        bookmarks: Vec<Bookmark>,
        update_calls: Arc<Mutex<Vec<i32>>>,
    }

    impl MockBookmarkRepository {
        fn new(bookmarks: Vec<Bookmark>) -> Self {
            Self {
                bookmarks,
                update_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_update_calls(&self) -> Vec<i32> {
            let calls = self.update_calls.lock().unwrap();
            calls.clone()
        }
    }

    impl BookmarkRepository for MockBookmarkRepository {
        fn get_by_id(
            &self,
            id: i32,
        ) -> Result<Option<Bookmark>, crate::domain::error::DomainError> {
            Ok(self.bookmarks.iter().find(|b| b.id() == Some(id)).cloned())
        }

        fn get_by_url(
            &self,
            url: &str,
        ) -> Result<Option<Bookmark>, crate::domain::error::DomainError> {
            Ok(self.bookmarks.iter().find(|b| b.url() == url).cloned())
        }

        fn search(
            &self,
            _query: &crate::domain::repositories::query::BookmarkQuery,
        ) -> Result<Vec<Bookmark>, crate::domain::error::DomainError> {
            Ok(self.bookmarks.clone())
        }

        fn get_all(&self) -> Result<Vec<Bookmark>, crate::domain::error::DomainError> {
            Ok(self.bookmarks.clone())
        }

        fn add(&self, _bookmark: &mut Bookmark) -> Result<(), crate::domain::error::DomainError> {
            Ok(())
        }

        fn update(&self, bookmark: &Bookmark) -> Result<(), crate::domain::error::DomainError> {
            if let Some(id) = bookmark.id() {
                let mut calls = self.update_calls.lock().unwrap();
                calls.push(id);
            }
            Ok(())
        }

        fn delete(&self, _id: i32) -> Result<bool, crate::domain::error::DomainError> {
            Ok(true)
        }

        fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, crate::domain::error::DomainError> {
            Ok(Vec::new())
        }

        fn get_related_tags(
            &self,
            _tag: &Tag,
        ) -> Result<Vec<(Tag, usize)>, crate::domain::error::DomainError> {
            Ok(Vec::new())
        }

        fn get_random(
            &self,
            _count: usize,
        ) -> Result<Vec<Bookmark>, crate::domain::error::DomainError> {
            Ok(Vec::new())
        }

        fn get_without_embeddings(
            &self,
        ) -> Result<Vec<Bookmark>, crate::domain::error::DomainError> {
            Ok(self.bookmarks.clone())
        }

        fn exists_by_url(&self, url: &str) -> Result<bool, crate::domain::error::DomainError> {
            Ok(self.bookmarks.iter().any(|b| b.url() == url))
        }
    }

    #[test]
    fn test_backfill_embeddings_dry_run() {
        // Create test bookmarks
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark1 = Bookmark::new(
            "https://example1.com",
            "Example 1",
            "Test description 1",
            tags.clone(),
        )
        .unwrap();

        let bookmark2 = Bookmark::new(
            "https://example2.com",
            "Example 2",
            "Test description 2",
            tags,
        )
        .unwrap();

        let bookmarks = vec![bookmark1, bookmark2];

        // Create mock repository
        let repo = MockBookmarkRepository::new(bookmarks);

        // Create service
        let service = EmbeddingService::new(repo);

        // Test dry run
        let count = service.backfill_embeddings(true).unwrap();

        // Should return count of bookmarks without embeddings
        assert_eq!(count, 2);

        // Should not have called update
        assert_eq!(service.repository.get_update_calls().len(), 0);
    }
}
