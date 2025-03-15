//! In-memory repository refactored to mirror the structure and logic of the SQLite repository.
//! We store an internal Vec of DbBookmark, converting to/from the domain model.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use rand::seq::IteratorRandom;
use anyhow::{anyhow, Result};

use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainError;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::tag::Tag;

/// Internal representation of a Bookmark analogous to a DB entity.
/// In practice, for an in-memory store, we could store the Bookmark directly,
/// but we replicate the SQLite approach with a separate struct.
#[derive(Clone, Debug)]
struct DbBookmark {
    id: i32,
    url: String,
    metadata: String, // used for the bookmark's title
    desc: String,
    tags: String,
    flags: i32, // used for "access_count" in domain
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    embedding: Option<Vec<u8>>,
    content_hash: Option<Vec<u8>>,
}

impl DbBookmark {
    fn to_domain(&self) -> Result<Bookmark, DomainError> {
        // Parse tags from stored format
        let parsed_tags = Tag::parse_tags(&self.tags)
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to parse tags: {e}")))?;

        // Construct domain bookmark
        // For consistency, we use from_storage
        Bookmark::from_storage(
            self.id,
            self.url.clone(),
            self.metadata.clone(),
            self.desc.clone(),
            self.tags.clone(),
            self.flags,
            self.created_at,
            self.updated_at,
        )
    }

    fn from_domain(b: &Bookmark) -> Self {
        DbBookmark {
            id: b.id().unwrap_or(0),
            url: b.url().to_string(),
            metadata: b.title().to_string(),
            desc: b.description().to_string(),
            tags: b.formatted_tags(),
            flags: b.access_count(),
            created_at: b.created_at(),
            updated_at: b.updated_at(),
            embedding: None,     // Not used in memory
            content_hash: None,  // Not used in memory
        }
    }
}

/// Thread-safe, in-memory repository for Bookmarks, refactored to match the structure of the SQLite repository.
pub struct InMemoryBookmarkRepository {
    storage: Arc<RwLock<HashMap<i32, DbBookmark>>>,
    next_id: Arc<RwLock<i32>>,
}

impl InMemoryBookmarkRepository {
    /// Create a new, empty repository.
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create a repository pre-loaded with Bookmarks.
    pub fn with_bookmarks(bookmarks: Vec<Bookmark>) -> Result<Self, DomainError> {
        let repo = Self::new();
        {
            let mut map = repo.storage.write().unwrap();
            let mut next_id = repo.next_id.write().unwrap();

            for bmk in bookmarks {
                let mut db = DbBookmark::from_domain(&bmk);
                // If domain bookmark has no ID, assign a new ID.
                if db.id == 0 {
                    db.id = *next_id;
                    *next_id += 1;
                } else {
                    // If the domain bookmark already has an ID, ensure next_id is correct.
                    if db.id >= *next_id {
                        *next_id = db.id + 1;
                    }
                }
                map.insert(db.id, db);
            }
        }
        Ok(repo)
    }

    /// Retrieve the next ID and increment.
    fn get_next_id(&self) -> i32 {
        let mut next_id = self.next_id.write().unwrap();
        let id = *next_id;
        *next_id += 1;
        id
    }

    /// In-memory approach for reindexing after we delete an ID.
    /// For each ID > deleted_id, we decrement by 1.
    fn reindex_after_delete(&self, deleted_id: i32) {
        let mut map = self.storage.write().unwrap();
        // Collect bookmarks with ID > deleted_id
        let mut to_reindex: Vec<(i32, DbBookmark)> = map
            .iter()
            .filter_map(|(k, v)| if *k > deleted_id { Some((*k, v.clone())) } else { None })
            .collect();

        // Sort by ID ascending just to be sure, though it should not matter.
        to_reindex.sort_by_key(|(id, _)| *id);

        // Remove them from map
        for (old_id, _) in &to_reindex {
            map.remove(old_id);
        }

        // Re-insert them with ID - 1
        for (old_id, mut db) in to_reindex {
            let new_id = old_id - 1;
            db.id = new_id;
            map.insert(new_id, db);
        }
    }

    /// Internal method to fetch all domain bookmarks.
    fn all_domain_bookmarks(&self) -> Vec<Bookmark> {
        let map = self.storage.read().unwrap();
        let mut result = Vec::new();
        for db in map.values() {
            if let Ok(b) = db.to_domain() {
                result.push(b);
            }
        }
        result
    }
}

impl Default for InMemoryBookmarkRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl BookmarkRepository for InMemoryBookmarkRepository {
    fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError> {
        let map = self.storage.read().unwrap();
        match map.get(&id) {
            Some(db) => Ok(Some(db.to_domain()?)),
            None => Ok(None),
        }
    }

    fn get_by_url(&self, url: &str) -> Result<Option<Bookmark>, DomainError> {
        let map = self.storage.read().unwrap();
        for db in map.values() {
            if db.url == url {
                return Ok(Some(db.to_domain()?));
            }
        }
        Ok(None)
    }

    fn search(&self, query: &BookmarkQuery) -> Result<Vec<Bookmark>, DomainError> {
        // If we have a specification, filter in memory using that spec.
        // Otherwise, just get all.
        let mut bookmarks = if let Some(_) = query.specification {
            // In memory we must load everything anyway.
            let all = self.all_domain_bookmarks();
            all.into_iter()
                .filter(|b| query.matches(b))
                .collect()
        } else {
            // No specification => all bookmarks
            self.all_domain_bookmarks()
        };

        // Apply sorting
        if let Some(sort_direction) = query.sort_by_date {
            match sort_direction {
                SortDirection::Ascending => {
                    bookmarks.sort_by(|a, b| a.updated_at().cmp(&b.updated_at()));
                }
                SortDirection::Descending => {
                    bookmarks.sort_by(|a, b| b.updated_at().cmp(&a.updated_at()));
                }
            }
        }

        // Apply offset
        if let Some(offset) = query.offset {
            if offset < bookmarks.len() {
                bookmarks = bookmarks.into_iter().skip(offset).collect();
            } else {
                bookmarks.clear();
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            bookmarks = bookmarks.into_iter().take(limit).collect();
        }

        Ok(bookmarks)
    }

    fn get_all(&self) -> Result<Vec<Bookmark>, DomainError> {
        Ok(self.all_domain_bookmarks())
    }

    fn add(&self, bookmark: &mut Bookmark) -> Result<(), DomainError> {
        let mut map = self.storage.write().unwrap();
        // If the bookmark has no ID, assign one.
        if bookmark.id().is_none() {
            let new_id = self.get_next_id();
            bookmark.set_id(new_id);
        }

        // Convert domain to DB
        let db = DbBookmark::from_domain(bookmark);
        // Insert
        map.insert(db.id, db);
        Ok(())
    }

    fn update(&self, bookmark: &Bookmark) -> Result<(), DomainError> {
        let id = bookmark.id().ok_or_else(|| {
            DomainError::BookmarkOperationFailed("Bookmark has no ID".to_string())
        })?;

        let mut map = self.storage.write().unwrap();
        if !map.contains_key(&id) {
            return Err(DomainError::BookmarkOperationFailed("Bookmark not found".to_string()));
        }

        // Overwrite existing entry
        let db = DbBookmark::from_domain(bookmark);
        map.insert(id, db);
        Ok(())
    }

    fn delete(&self, id: i32) -> Result<bool, DomainError> {
        let mut map = self.storage.write().unwrap();
        if map.remove(&id).is_none() {
            // Not found
            return Ok(false);
        }

        // Reindex
        drop(map); // drop the write lock
        self.reindex_after_delete(id);
        Ok(true)
    }

    fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, DomainError> {
        let bookmarks = self.all_domain_bookmarks();
        let mut counts = HashMap::new();
        for b in &bookmarks {
            for t in b.tags() {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
        let mut result: Vec<(Tag, usize)> = counts.into_iter().collect();
        // Sort by frequency desc
        result.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(result)
    }

    fn get_related_tags(&self, tag: &Tag) -> Result<Vec<(Tag, usize)>, DomainError> {
        let bookmarks = self.all_domain_bookmarks();
        let mut counts = HashMap::new();

        // Find bookmarks containing the given tag
        let with_tag: Vec<Bookmark> = bookmarks
            .into_iter()
            .filter(|b| b.tags().contains(tag))
            .collect();

        // Count co-occurrences
        for b in &with_tag {
            for t in b.tags() {
                if t != tag {
                    *counts.entry(t.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut result: Vec<(Tag, usize)> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(result)
    }

    fn get_random(&self, count: usize) -> Result<Vec<Bookmark>, DomainError> {
        use rand::thread_rng;
        let bookmarks = self.all_domain_bookmarks();
        let mut rng = thread_rng();
        let chosen = bookmarks.into_iter().choose_multiple(&mut rng, count);
        Ok(chosen)
    }

    fn get_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError> {
        // We do not store embeddings in memory, so assume all are without embeddings.
        // If we had them, we'd filter here.
        Ok(self.all_domain_bookmarks())
    }

    fn exists_by_url(&self, url: &str) -> Result<bool, DomainError> {
        Ok(self.get_by_url(url)?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    use crate::domain::bookmark::Bookmark;
    use crate::domain::tag::Tag;
    use crate::domain::repositories::query::{BookmarkQuery, TextSearchSpecification, AllTagsSpecification};

    fn create_test_bookmark(id: Option<i32>, url: &str, title: &str, tags: Vec<&str>) -> Bookmark {
        let tag_set: HashSet<Tag> = tags.into_iter().map(|t| Tag::new(t).unwrap()).collect();
        let mut bmk = Bookmark::new(url, title, "Desc", tag_set).unwrap();
        if let Some(i) = id {
            bmk.set_id(i);
        }
        bmk
    }

    #[test]
    fn test_add_and_get_by_id() {
        let repo = InMemoryBookmarkRepository::new();
        let mut bookmark = create_test_bookmark(None, "https://example.com", "Example", vec!["test"]);
        repo.add(&mut bookmark).unwrap();

        let id = bookmark.id().unwrap();
        let fetched = repo.get_by_id(id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().url(), "https://example.com");
    }

    #[test]
    fn test_delete_and_reindex() {
        let repo = InMemoryBookmarkRepository::new();

        let mut b1 = create_test_bookmark(Some(1), "https://one.com", "One", vec![]);
        let mut b2 = create_test_bookmark(Some(2), "https://two.com", "Two", vec![]);
        let mut b3 = create_test_bookmark(Some(3), "https://three.com", "Three", vec![]);
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();
        repo.add(&mut b3).unwrap();

        // Check all
        assert!(repo.get_by_id(1).unwrap().is_some());
        assert!(repo.get_by_id(2).unwrap().is_some());
        assert!(repo.get_by_id(3).unwrap().is_some());

        // Delete ID 2
        let result = repo.delete(2).unwrap();
        assert!(result);

        // Now only ID 1 and ID 2 remain
        assert!(repo.get_by_id(1).unwrap().is_some());
        // The old ID 3 is reindexed to 2
        assert!(repo.get_by_id(3).unwrap().is_none());
        assert!(repo.get_by_id(2).unwrap().is_some());
        // That new ID 2 should have the old b3's url
        assert_eq!(repo.get_by_id(2).unwrap().unwrap().url(), "https://three.com");
    }

    #[test]
    fn test_search_with_spec() {
        let repo = InMemoryBookmarkRepository::new();
        let mut b1 = create_test_bookmark(None, "https://rust-lang.org", "Rust", vec!["programming"]);
        let mut b2 = create_test_bookmark(None, "https://python.org", "Python", vec!["programming"]);
        let mut b3 = create_test_bookmark(None, "https://recipe.com", "Recipe", vec!["food"]);
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();
        repo.add(&mut b3).unwrap();

        let query = BookmarkQuery::new()
            .with_specification(TextSearchSpecification::new("programming".into()));
        let results = repo.search(&query).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_all_tags() {
        let repo = InMemoryBookmarkRepository::new();
        let mut b1 = create_test_bookmark(None, "https://one.com", "One", vec!["tag1", "tag2"]);
        let mut b2 = create_test_bookmark(None, "https://two.com", "Two", vec!["tag2", "tag3"]);
        repo.add(&mut b1).unwrap();
        repo.add(&mut b2).unwrap();

        let tags = repo.get_all_tags().unwrap();
        // tags: tag2 -> 2, tag1 -> 1, tag3 -> 1
        assert_eq!(tags.len(), 3);
        let (t, count) = &tags[0];
        assert_eq!(t.value(), "tag2");
        assert_eq!(*count, 2);
    }
}
