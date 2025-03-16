// src/infrastructure/repositories/sqlite/bookmark_repository.rs

use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Integer, Text};
use tracing::{error, instrument};

use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::tag::Tag;
use crate::domain::error::DomainError;
use crate::adapter::dal::schema::bookmarks::{self, dsl};
use super::connection::{ConnectionPool, PooledConnection};
use super::error::{SqliteRepositoryError, SqliteResult};

/// Implementation of BookmarkRepository for SQLite database
#[derive(Clone)]
pub struct SqliteBookmarkRepository {
    pool: ConnectionPool,
}

impl SqliteBookmarkRepository {
    /// Create a new SQLite repository with the provided connection pool
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    /// Create a new SQLite repository with the provided database URL
    pub fn from_url(database_url: &str) -> SqliteResult<Self> {
        let pool = super::connection::init_pool(database_url)?;
        Ok(Self { pool })
    }

    /// Get a connection from the pool
    fn get_connection(&self) -> SqliteResult<PooledConnection> {
        self.pool.get()
            .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))
    }

    /// Convert a database model to a domain entity
    fn to_domain_model(&self, db_bookmark: DbBookmark) -> SqliteResult<Bookmark> {
        // Parse tags from the stored format
        let _tags = Tag::parse_tags(&db_bookmark.tags)
            .map_err(|e| SqliteRepositoryError::ConversionError(format!("Failed to parse tags for bookmark ID {}: {}", db_bookmark.id, e)))?;

        // Convert stored timestamp to DateTime<Utc>
        let created_at = DateTime::from_naive_utc_and_offset(
            db_bookmark.last_update_ts,
            Utc
        );

        // Use same timestamp for updated_at for now
        let updated_at = created_at;

        // Create bookmark from storage data
        Bookmark::from_storage(
            db_bookmark.id,
            db_bookmark.URL,
            db_bookmark.metadata,
            db_bookmark.desc,
            db_bookmark.tags,
            db_bookmark.flags,
            created_at,
            updated_at,
        ).map_err(|e| SqliteRepositoryError::ConversionError(format!("Failed to create domain bookmark from DB model for ID {}: {}", db_bookmark.id, e)))
    }

    /// Convert a domain entity to a database model
    fn to_db_model(&self, bookmark: &Bookmark) -> DbBookmarkChanges {
        DbBookmarkChanges {
            URL: bookmark.url().to_string(),
            metadata: bookmark.title().to_string(),
            tags: bookmark.formatted_tags(),
            desc: bookmark.description().to_string(),
            flags: bookmark.access_count(),
            embedding: None, // We'll handle this separately
            content_hash: None, // We'll handle this separately
        }
    }

    /// Execute a query that might return bookmark IDs
    fn execute_id_query(&self, query: &str, params: Vec<String>) -> SqliteResult<Vec<i32>> {
        let mut conn = self.get_connection()?;

        // Build the query with a dynamic approach for parameters
        let query_builder = diesel::sql_query(query);

        // Since we can't reassign query_builder due to changing types,
        // we need to handle parameter binding differently
        let results: Vec<IdResult> = match params.len() {
            0 => query_builder.load(&mut conn),
            1 => query_builder.bind::<Text, _>(&params[0]).load(&mut conn),
            2 => query_builder
                .bind::<Text, _>(&params[0])
                .bind::<Text, _>(&params[1])
                .load(&mut conn),
            3 => query_builder
                .bind::<Text, _>(&params[0])
                .bind::<Text, _>(&params[1])
                .bind::<Text, _>(&params[2])
                .load(&mut conn),
            _ => return Err(SqliteRepositoryError::InvalidParameter("Too many parameters for query".to_string()))
        }.map_err(SqliteRepositoryError::DatabaseError)?;

        // Extract IDs
        Ok(results.into_iter().map(|result| result.id).collect())
    }

    /// Build SQL condition from specifications
    fn build_sql_conditions(&self, query: &BookmarkQuery) -> (String, Vec<String>) {
        let conditions: Vec<String> = Vec::new();
        let params = Vec::new();

        // If no specification, return empty conditions
        if query.specification.is_none() {
            return (String::new(), params);
        }

        // For simplicity, we'll handle common query patterns directly
        // This could be expanded to handle more complex specifications

        // Return the built conditions
        let conditions_str = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (conditions_str, params)
    }

    /// Generate a SQL ORDER BY clause from query sorting parameters
    fn build_sql_order_by(&self, query: &BookmarkQuery) -> String {
        if let Some(direction) = query.sort_by_date {
            match direction {
                SortDirection::Ascending => "ORDER BY last_update_ts ASC".to_string(),
                SortDirection::Descending => "ORDER BY last_update_ts DESC".to_string(),
            }
        } else {
            "ORDER BY id ASC".to_string() // Default ordering
        }
    }

    /// Generate SQL LIMIT and OFFSET clauses
    fn build_sql_pagination(&self, query: &BookmarkQuery) -> String {
        let mut pagination = String::new();

        if let Some(limit) = query.limit {
            pagination.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = query.offset {
            pagination.push_str(&format!(" OFFSET {}", offset));
        }

        pagination
    }
}

impl BookmarkRepository for SqliteBookmarkRepository {
    fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        let result = dsl::bookmarks
            .filter(dsl::id.eq(id))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get bookmark with ID {}: {}", id, e)
            ))?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)
                    .map_err(DomainError::from)?;
                Ok(Some(bookmark))
            },
            None => Ok(None),
        }
    }

    fn get_by_url(&self, url: &str) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // Escape special characters in URL for SQLite query
        let escaped_url = url.replace('\'', "''");

        let result = dsl::bookmarks
            .filter(dsl::URL.eq(escaped_url))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get bookmark with URL {}: {}", url, e)
            ))?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)
                    .map_err(DomainError::from)?;
                Ok(Some(bookmark))
            },
            None => Ok(None),
        }
    }

    fn search(&self, query: &BookmarkQuery) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // If there's a specification, we need more complex handling
        if query.specification.is_some() {
            // Get all bookmarks and filter them in-memory using the specification
            let all_bookmarks = self.get_all()?;

            let filtered = all_bookmarks.into_iter()
                .filter(|bookmark| query.matches(bookmark))
                .collect::<Vec<_>>();

            // Apply sorting
            let mut sorted = filtered;
            if let Some(sort_direction) = query.sort_by_date {
                match sort_direction {
                    SortDirection::Ascending => {
                        sorted.sort_by_key(|a| a.updated_at());
                    },
                    SortDirection::Descending => {
                        sorted.sort_by(|a, b| b.updated_at().cmp(&a.updated_at()));
                    },
                }
            }

            // Apply offset and limit
            let mut result = sorted;
            if let Some(offset) = query.offset {
                if offset < result.len() {
                    result = result.into_iter().skip(offset).collect();
                } else {
                    result = Vec::new();
                }
            }

            if let Some(limit) = query.limit {
                result = result.into_iter().take(limit).collect();
            }

            return Ok(result);
        }

        // If no specification, just apply sorting and pagination
        // Build a query using the BoxedSelectStatement
        let mut query_builder = dsl::bookmarks.into_boxed();

        // Apply sorting
        if let Some(sort_direction) = query.sort_by_date {
            match sort_direction {
                SortDirection::Ascending => {
                    query_builder = query_builder.order(dsl::last_update_ts.asc());
                },
                SortDirection::Descending => {
                    query_builder = query_builder.order(dsl::last_update_ts.desc());
                },
            }
        }

        // Apply pagination
        if let Some(limit) = query.limit {
            query_builder = query_builder.limit(limit as i64);
        }

        if let Some(offset) = query.offset {
            query_builder = query_builder.offset(offset as i64);
        }

        // Execute query
        let db_bookmarks = query_builder
            .load::<DbBookmark>(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to search bookmarks: {}", e)
            ))?;

        // Convert to domain models
        let bookmarks = db_bookmarks.into_iter()
            .filter_map(|db_bookmark| {
                match self.to_domain_model(db_bookmark) {
                    Ok(bookmark) => Some(bookmark),
                    Err(e) => {
                        error!("Failed to convert bookmark: {}", e);
                        None
                    }
                }
            })
            .collect();

        Ok(bookmarks)
    }

    fn get_all(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        let db_bookmarks = dsl::bookmarks
            .load::<DbBookmark>(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get all bookmarks: {}", e)
            ))?;

        let mut bookmarks = Vec::new();
        for db_bookmark in db_bookmarks {
            match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => bookmarks.push(bookmark),
                Err(e) => error!("Failed to convert bookmark: {}", e),
            }
        }

        Ok(bookmarks)
    }

    fn add(&self, bookmark: &mut Bookmark) -> Result<(), DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // Begin transaction
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let db_bookmark = NewBookmark {
                URL: bookmark.url().to_string(),
                metadata: bookmark.title().to_string(),
                tags: bookmark.formatted_tags(),
                desc: bookmark.description().to_string(),
                flags: bookmark.access_count(),
                embedding: None, // Will be handled separately
                content_hash: None, // Will be handled separately
            };

            // Insert the bookmark
            let result = diesel::insert_into(dsl::bookmarks)
                .values(&db_bookmark)
                .execute(conn)?;

            if result == 0 {
                return Err(diesel::result::Error::NotFound);
            }

            // Get the inserted ID
            let id = diesel::select(diesel::dsl::sql::<Integer>("last_insert_rowid()"))
                .get_result::<i32>(conn)?;

            // Update the domain entity with the new ID
            bookmark.set_id(id);

            Ok(())
        })
        .map_err(|e| DomainError::BookmarkOperationFailed(
            format!("Failed to add bookmark: {}", e)
        ))?;

        Ok(())
    }

    fn update(&self, bookmark: &Bookmark) -> Result<(), DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        let id = bookmark.id().ok_or_else(|| DomainError::BookmarkOperationFailed(
            "Bookmark has no ID".to_string()
        ))?;

        // Create the update data
        let changes = self.to_db_model(bookmark);

        // Update the bookmark
        let result = diesel::update(dsl::bookmarks.filter(dsl::id.eq(id)))
            .set(&changes)
            .execute(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to update bookmark with ID {}: {}", id, e)
            ))?;

        if result == 0 {
            return Err(DomainError::BookmarkNotFound(id.to_string()));
        }

        Ok(())
    }

    fn delete(&self, id: i32) -> Result<bool, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // Begin transaction
        conn.transaction::<bool, diesel::result::Error, _>(|conn| {
            // Delete the bookmark
            let result = diesel::delete(dsl::bookmarks.filter(dsl::id.eq(id)))
                .execute(conn)?;

            if result == 0 {
                return Ok(false); // No bookmark was deleted
            }

            // Update IDs of remaining bookmarks to maintain sequential IDs
            diesel::sql_query(
                "UPDATE bookmarks SET id = id - 1 WHERE id > ?"
            )
            .bind::<Integer, _>(id)
            .execute(conn)?;

            // Return success
            Ok(true)
        })
        .map_err(|e| DomainError::BookmarkOperationFailed(
            format!("Failed to delete bookmark with ID {}: {}", id, e)
        ))
    }

    #[instrument(skip(self), level = "trace")]
    fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // SQL query to extract tags and their frequencies
        let query = "
            WITH RECURSIVE split(tags, rest) AS (
                SELECT '', tags || ','
                FROM bookmarks
                UNION ALL
                SELECT substr(rest, 0, instr(rest, ',')),
                       substr(rest, instr(rest, ',') + 1)
                FROM split
                WHERE rest <> '')
            SELECT tags as tag, count(tags) as n
            FROM split
            WHERE tags <> ''
            GROUP BY tags
            ORDER BY 2 DESC;
        ";

        let tag_frequencies: Vec<TagsFrequency> = sql_query(query)
            .load(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get all tags: {}", e)
            ))?;

        // Convert to domain model
        let mut result = Vec::new();
        for tf in tag_frequencies {
            match Tag::new(&tf.tag) {
                Ok(tag) => result.push((tag, tf.n as usize)),
                Err(e) => error!("Failed to create tag '{}': {}", tf.tag, e),
            }
        }

        Ok(result)
    }

    fn get_related_tags(&self, tag: &Tag) -> Result<Vec<(Tag, usize)>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        let search_tag = format!("%,{},%", tag.value());

        // SQL query to find tags that co-occur with the given tag
        let query = "
            WITH RECURSIVE split(tags, rest) AS (
                SELECT '', tags || ','
                FROM bookmarks
                WHERE tags LIKE ?
                UNION ALL
                SELECT substr(rest, 0, instr(rest, ',')),
                       substr(rest, instr(rest, ',') + 1)
                FROM split
                WHERE rest <> '')
            SELECT tags as tag, count(tags) as n
            FROM split
            WHERE tags <> ''
            GROUP BY tags
            ORDER BY 2 DESC;
        ";

        let tag_frequencies: Vec<TagsFrequency> = sql_query(query)
            .bind::<Text, _>(search_tag)
            .load(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get related tags for '{}': {}", tag.value(), e)
            ))?;

        // Convert to domain model (excluding the search tag itself)
        let mut result = Vec::new();
        for tf in tag_frequencies {
            // Skip the tag we're searching for
            if tf.tag == tag.value() {
                continue;
            }

            match Tag::new(&tf.tag) {
                Ok(related_tag) => result.push((related_tag, tf.n as usize)),
                Err(e) => error!("Failed to create tag '{}': {}", tf.tag, e),
            }
        }

        Ok(result)
    }

    fn get_random(&self, count: usize) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        // First get all IDs
        #[derive(QueryableByName, Debug)]
        #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
        struct RandomId {
            #[diesel(sql_type = Integer)]
            pub id: i32,
        }

        // Get random IDs
        let random_ids: Vec<RandomId> = diesel::sql_query(format!(
            "SELECT id FROM bookmarks ORDER BY RANDOM() LIMIT {}", count
        ))
        .load(&mut conn)
        .map_err(|e| DomainError::BookmarkOperationFailed(
            format!("Failed to get {} random bookmark IDs: {}", count, e)
        ))?;

        // If no bookmarks found, return empty vec
        if random_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Extract IDs
        let ids: Vec<i32> = random_ids.into_iter().map(|r| r.id).collect();

        // Load bookmarks using query DSL
        let db_bookmarks = dsl::bookmarks
            .filter(dsl::id.eq_any(ids))
            .load::<DbBookmark>(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to load random bookmarks: {}", e)
            ))?;

        // Convert to domain models
        let bookmarks = db_bookmarks.into_iter()
            .filter_map(|db_bookmark| {
                match self.to_domain_model(db_bookmark) {
                    Ok(bookmark) => Some(bookmark),
                    Err(e) => {
                        error!("Failed to convert bookmark: {}", e);
                        None
                    }
                }
            })
            .collect();

        Ok(bookmarks)
    }

    fn get_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection().map_err(DomainError::from)?;

        let db_bookmarks = dsl::bookmarks
            .filter(dsl::embedding.is_null())
            .load::<DbBookmark>(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(
                format!("Failed to get bookmarks without embeddings: {}", e)
            ))?;

        let mut bookmarks = Vec::new();
        for db_bookmark in db_bookmarks {
            match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => bookmarks.push(bookmark),
                Err(e) => error!("Failed to convert bookmark: {}", e),
            }
        }

        Ok(bookmarks)
    }

    fn exists_by_url(&self, url: &str) -> Result<bool, crate::domain::error::DomainError> {
        self.get_by_url(url).map(|result| result.is_some())
    }
}

/// Database model for bookmarks
#[derive(Queryable, Identifiable, QueryableByName, Debug)]
#[diesel(table_name = bookmarks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct DbBookmark {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[allow(non_snake_case)]
    #[diesel(sql_type = Text)]
    pub URL: String,
    #[diesel(sql_type = Text)]
    pub metadata: String,
    #[diesel(sql_type = Text)]
    pub tags: String,
    #[diesel(sql_type = Text)]
    pub desc: String,
    #[diesel(sql_type = Integer)]
    pub flags: i32,
    #[diesel(sql_type = diesel::sql_types::Timestamp)]
    pub last_update_ts: NaiveDateTime,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Binary>)]
    pub embedding: Option<Vec<u8>>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Binary>)]
    pub content_hash: Option<Vec<u8>>,
}

/// Changes for updating a bookmark
#[derive(AsChangeset, Debug)]
#[diesel(table_name = bookmarks)]
struct DbBookmarkChanges {
    #[allow(non_snake_case)]
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
}

/// New bookmark for insertion
#[derive(Insertable, Debug)]
#[diesel(table_name = bookmarks)]
struct NewBookmark {
    #[allow(non_snake_case)]
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
}

/// ID result from queries
#[derive(QueryableByName, Debug)]
struct IdResult {
    #[diesel(sql_type = Integer)]
    pub id: i32,
}

/// Tags frequency for aggregation queries
#[derive(QueryableByName, Debug)]
struct TagsFrequency {
    #[diesel(sql_type = Text)]
    pub tag: String,

    #[diesel(sql_type = Integer)]
    pub n: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::query::TextSearchSpecification;
    use crate::infrastructure::repositories::sqlite::connection::init_db;
    use std::collections::HashSet;
    

    fn setup_test_db() -> Result<(SqliteBookmarkRepository, String), DomainError> {
        // Create a temporary file instead of a path in a temporary directory
        // This ensures we have proper write permissions
        let temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to create temp file: {}", e)))?;
        let db_path = temp_file.path().to_str().unwrap().to_string();

        // Keep the temp_file from being deleted while we're using it
        // (We don't need to use it directly, but it shouldn't be dropped)
        std::mem::forget(temp_file);

        // Initialize pool with a unique path for each test
        let pool = super::super::connection::init_pool(&db_path)
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to initialize pool: {}", e)))?;

        // Initialize DB schema
        let mut conn = pool.get()
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to get connection: {}", e)))?;
        init_db(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to initialize DB: {}", e)))?;

        // Clean any existing data
        diesel::sql_query("DELETE FROM bookmarks")
            .execute(&mut conn)
            .map_err(|e| DomainError::BookmarkOperationFailed(format!("Failed to clean table: {}", e)))?;

        Ok((SqliteBookmarkRepository::new(pool), db_path))
    }

    fn create_test_bookmark(title: &str, url: &str, tags: Vec<&str>) -> Result<Bookmark, DomainError> {
        let tag_set: HashSet<Tag> = tags.into_iter()
            .map(Tag::new)
            .collect::<std::result::Result<HashSet<_>, _>>()?;

        Bookmark::new(url, title, "Test description", tag_set)
    }

    #[test]
    fn test_add_and_get_by_id() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Create and add a bookmark
        let mut bookmark = create_test_bookmark(
            "Test Bookmark",
            "https://example.com",
            vec!["test", "example"]
        )?;

        repo.add(&mut bookmark)?;

        // Check that ID was set
        assert!(bookmark.id().is_some());

        // Get the bookmark by ID
        let retrieved = repo.get_by_id(bookmark.id().unwrap())?;

        // Verify it was retrieved correctly
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title(), "Test Bookmark");
        assert_eq!(retrieved.url(), "https://example.com");

        // Check tags
        assert_eq!(retrieved.tags().len(), 2);
        assert!(retrieved.tags().contains(&Tag::new("test")?));
        assert!(retrieved.tags().contains(&Tag::new("example")?));

        Ok(())
    }

    #[test]
    fn test_get_by_url() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Create and add a bookmark
        let mut bookmark = create_test_bookmark(
            "URL Test",
            "https://url-test.com",
            vec!["url", "test"]
        )?;

        repo.add(&mut bookmark)?;

        // Get the bookmark by URL
        let retrieved = repo.get_by_url("https://url-test.com")?;

        // Verify it was retrieved correctly
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title(), "URL Test");

        // Test with nonexistent URL
        let not_found = repo.get_by_url("https://nonexistent.com")?;
        assert!(not_found.is_none());

        Ok(())
    }

    #[test]
    fn test_update() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Create and add a bookmark
        let mut bookmark = create_test_bookmark(
            "Original Title",
            "https://update-test.com",
            vec!["original"]
        )?;

        repo.add(&mut bookmark)?;
        let id = bookmark.id().unwrap();

        // Update the bookmark
        let mut updated_tags = HashSet::new();
        updated_tags.insert(Tag::new("updated")?);

        let mut updated = bookmark.clone();
        updated.update("Updated Title".to_string(), "Updated Description".to_string());
        updated.set_tags(updated_tags)?;

        repo.update(&updated)?;

        // Get the updated bookmark
        let retrieved = repo.get_by_id(id)?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title(), "Updated Title");
        assert_eq!(retrieved.description(), "Updated Description");
        assert_eq!(retrieved.tags().len(), 1);
        assert!(retrieved.tags().contains(&Tag::new("updated")?));

        Ok(())
    }

    #[test]
    fn test_delete_and_reindex() -> Result<(), DomainError> {
        // Create a clean repository
        let (repo, _) = setup_test_db()?;

        // Add two bookmarks
        let mut bookmark1 = create_test_bookmark(
            "First Bookmark",
            "https://first.com",
            vec!["test"]
        )?;
        repo.add(&mut bookmark1)?;

        let mut bookmark2 = create_test_bookmark(
            "Second Bookmark",
            "https://second.com",
            vec!["test"]
        )?;
        repo.add(&mut bookmark2)?;

        // Directly query all bookmarks to see their state
        let all = repo.get_all()?;
        assert_eq!(all.len(), 2, "Should have 2 bookmarks before deletion");
        assert_eq!(all[0].id(), Some(1), "First bookmark should have ID 1");
        assert_eq!(all[1].id(), Some(2), "Second bookmark should have ID 2");

        // Delete first bookmark
        repo.delete(1)?;

        // Get all bookmarks again to see the updated state
        let updated = repo.get_all()?;
        assert_eq!(updated.len(), 1, "Should have 1 bookmark after deletion");

        // The remaining bookmark should have ID 1 and be the second bookmark
        assert_eq!(updated[0].id(), Some(1), "Remaining bookmark should have ID 1");
        assert_eq!(updated[0].url(), "https://second.com", "Remaining bookmark should be the second one");

        Ok(())
    }

    #[test]
    fn test_search() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add test bookmarks
        let mut bookmark1 = create_test_bookmark(
            "Rust Programming",
            "https://rust-lang.org",
            vec!["programming", "rust"]
        )?;

        let mut bookmark2 = create_test_bookmark(
            "Python Guide",
            "https://python.org",
            vec!["programming", "python"]
        )?;

        let mut bookmark3 = create_test_bookmark(
            "Cooking Recipes",
            "https://recipes.com",
            vec!["cooking", "food"]
        )?;

        repo.add(&mut bookmark1)?;
        repo.add(&mut bookmark2)?;
        repo.add(&mut bookmark3)?;

        // Search using specification
        let query = BookmarkQuery::new()
            .with_specification(TextSearchSpecification::new("programming".to_string()));

        let results = repo.search(&query)?;
        assert_eq!(results.len(), 2);

        // Verify programming-related bookmarks are returned
        let urls: Vec<&str> = results.iter().map(|b| b.url()).collect();
        assert!(urls.contains(&"https://rust-lang.org"));
        assert!(urls.contains(&"https://python.org"));
        assert!(!urls.contains(&"https://recipes.com"));

        Ok(())
    }

    #[test]
    fn test_get_all_tags() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add bookmarks with various tags
        let mut bookmark1 = create_test_bookmark(
            "Test 1",
            "https://example1.com",
            vec!["tag1", "tag2"]
        )?;

        let mut bookmark2 = create_test_bookmark(
            "Test 2",
            "https://example2.com",
            vec!["tag2", "tag3"]
        )?;

        repo.add(&mut bookmark1)?;
        repo.add(&mut bookmark2)?;

        // Get all tags
        let tags = repo.get_all_tags()?;

        // Verify tags and counts
        assert_eq!(tags.len(), 3);

        // Find tag2 which should have count 2
        let tag2 = tags.iter().find(|(tag, _)| tag.value() == "tag2");
        assert!(tag2.is_some());
        assert_eq!(tag2.unwrap().1, 2);

        // tag1 and tag3 should have count 1
        let tag1 = tags.iter().find(|(tag, _)| tag.value() == "tag1");
        assert!(tag1.is_some());
        assert_eq!(tag1.unwrap().1, 1);

        let tag3 = tags.iter().find(|(tag, _)| tag.value() == "tag3");
        assert!(tag3.is_some());
        assert_eq!(tag3.unwrap().1, 1);

        Ok(())
    }

    #[test]
    fn test_get_related_tags() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add bookmarks with related tags
        let mut bookmark1 = create_test_bookmark(
            "Test 1",
            "https://example1.com",
            vec!["common", "related1"]
        )?;

        let mut bookmark2 = create_test_bookmark(
            "Test 2",
            "https://example2.com",
            vec!["common", "related2"]
        )?;

        let mut bookmark3 = create_test_bookmark(
            "Test 3",
            "https://example3.com",
            vec!["unrelated"]
        )?;

        repo.add(&mut bookmark1)?;
        repo.add(&mut bookmark2)?;
        repo.add(&mut bookmark3)?;

        // Get tags related to "common"
        let common_tag = Tag::new("common")?;
        let related = repo.get_related_tags(&common_tag)?;

        // Should find related1 and related2
        assert_eq!(related.len(), 2);

        let tag_values: HashSet<String> = related.iter()
            .map(|(tag, _)| tag.value().to_string())
            .collect();

        assert!(tag_values.contains("related1"));
        assert!(tag_values.contains("related2"));
        assert!(!tag_values.contains("unrelated"));

        Ok(())
    }

    #[test]
    fn test_get_random() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add multiple bookmarks
        for i in 1..=5 {
            let mut bookmark = create_test_bookmark(
                &format!("Test {}", i),
                &format!("https://example{}.com", i),
                vec!["test"]
            )?;
            repo.add(&mut bookmark)?;
        }

        // Get 2 random bookmarks
        let random = repo.get_random(2)?;

        // Verify count
        assert_eq!(random.len(), 2);

        // Verify they're valid bookmarks
        for bookmark in &random {
            assert!(bookmark.id().is_some());
            assert!(bookmark.url().starts_with("https://example"));
        }

        Ok(())
    }

    #[test]
    fn test_get_without_embeddings() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add bookmarks (all should have null embeddings in this test)
        for i in 1..=3 {
            let mut bookmark = create_test_bookmark(
                &format!("Test {}", i),
                &format!("https://example{}.com", i),
                vec!["test"]
            )?;
            repo.add(&mut bookmark)?;
        }

        // Get bookmarks without embeddings
        let bookmarks = repo.get_without_embeddings()?;

        // All bookmarks should be returned
        assert_eq!(bookmarks.len(), 3);

        Ok(())
    }

    #[test]
    fn test_exists_by_url() -> Result<(), DomainError> {
        let (repo, _) = setup_test_db()?;

        // Add a test bookmark
        let mut bookmark = create_test_bookmark(
            "Test Bookmark",
            "https://exists-test.com",
            vec!["test"]
        )?;
        repo.add(&mut bookmark)?;

        // Check existing URL
        let exists = repo.exists_by_url("https://exists-test.com")?;
        assert!(exists);

        // Check non-existing URL
        let not_exists = repo.exists_by_url("https://does-not-exist.com")?;
        assert!(!not_exists);

        Ok(())
    }
}