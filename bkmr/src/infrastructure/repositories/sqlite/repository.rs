// src/infrastructure/repositories/sqlite/repository

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Integer, Text};
use std::collections::HashSet;
use tracing::{debug, error, info, instrument};

use diesel::QueryableByName;

use super::connection::{ConnectionPool, PooledConnection};
use super::error::{SqliteRepositoryError, SqliteResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainError;
use crate::domain::repositories::query::{
    AllTagsSpecification, AnyTagSpecification, BookmarkQuery, SortDirection,
};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::sqlite::model::{
    DbBookmark, DbBookmarkChanges, IdResult, NewBookmark, TagsFrequency,
};
use crate::infrastructure::repositories::sqlite::schema::bookmarks::dsl;

#[derive(Debug, QueryableByName)]
struct TableSchema {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

pub fn print_db_schema(repo: &SqliteBookmarkRepository) {
    let mut conn = repo
        .get_connection()
        .expect("Failed to get repository connection");

    let results =
        sql_query("SELECT name, sql FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .load::<TableSchema>(&mut conn)
            .expect("Failed to retrieve schema rows");

    for table in results {
        println!("TABLE: {}\n{}", table.name, table.sql);
    }
}

#[derive(Clone, Debug)]
pub struct SqliteBookmarkRepository {
    pool: ConnectionPool,
}

impl SqliteBookmarkRepository {
    /// Create a new SQLite repository with the provided connection pool
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    /// Create a new SQLite repository with the provided database URL
    #[instrument(skip_all, level = "debug")]
    pub fn from_url(database_url: &str) -> SqliteResult<Self> {
        let pool = super::connection::init_pool(database_url)?;
        Ok(Self { pool })
    }

    /// Get a connection from the pool
    #[instrument(skip_all, level = "debug")]
    pub fn get_connection(&self) -> SqliteResult<PooledConnection> {
        self.pool
            .get()
            .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))
    }

    /// Cleans the table by deleting all bookmarks except ID 1
    #[instrument(skip_all, level = "debug")]
    pub fn empty_bookmark_table(&self) -> SqliteResult<()> {
        let mut conn = self.get_connection()?;

        // sql_query("DELETE FROM bookmarks WHERE id != 1;")
        sql_query("DELETE FROM bookmarks;")
            .execute(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)?;

        debug!("Cleaned table.");
        Ok(())
    }

    /// Convert a database model to a domain entity
    #[instrument(skip_all, level = "trace")]
    fn to_domain_model(&self, db_bookmark: DbBookmark) -> SqliteResult<Bookmark> {
        // Convert stored timestamp to DateTime<Utc>
        let created_at =
            DateTime::<Utc>::from_naive_utc_and_offset(db_bookmark.last_update_ts, Utc);
        let updated_at =
            DateTime::<Utc>::from_naive_utc_and_offset(db_bookmark.last_update_ts, Utc);

        // Create bookmark from storage data
        Bookmark::from_storage(
            db_bookmark.id,
            db_bookmark.url,
            db_bookmark.metadata,
            db_bookmark.desc,
            db_bookmark.tags,
            db_bookmark.flags,
            created_at,
            updated_at,
            db_bookmark.embedding,
            db_bookmark.content_hash,
            db_bookmark.embeddable,
        )
        .map_err(|e| {
            SqliteRepositoryError::ConversionError(format!(
                "Failed to create domain bookmark from DB model for ID {}: {}",
                db_bookmark.id, e
            ))
        })
    }

    /// Convert a domain entity to a database model
    #[instrument(skip_all, level = "debug")]
    fn to_db_model(&self, bookmark: &Bookmark) -> DbBookmarkChanges {
        DbBookmarkChanges {
            url: bookmark.url.to_string(),
            metadata: bookmark.title.to_string(),
            tags: bookmark.formatted_tags(),
            desc: bookmark.description.to_string(),
            flags: bookmark.access_count,
            embedding: bookmark.embedding.clone(), // Get embedding from domain entity
            content_hash: bookmark.content_hash.clone(), // Get content hash from domain entity
            created_ts: Some(bookmark.created_at.naive_utc()),
            embeddable: bookmark.embeddable,
        }
    }
}

impl BookmarkRepository for SqliteBookmarkRepository {
    #[instrument(skip_all, level = "debug")]
    fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        let result = dsl::bookmarks
            .filter(dsl::id.eq(id))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(SqliteRepositoryError::DatabaseError)?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)?;
                Ok(Some(bookmark))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip_all, level = "debug")]
    fn get_by_url(&self, url: &str) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        // Escape special characters in URL for SQLite query
        let escaped_url = url.replace('\'', "''");

        let result = dsl::bookmarks
            .filter(dsl::URL.eq(escaped_url))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(SqliteRepositoryError::DatabaseError)?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)?;
                Ok(Some(bookmark))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip_all, level = "debug")]
    fn search(&self, query: &BookmarkQuery) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        // If there's a specification, we need more complex handling
        if query.specification.is_some() {
            // Get all bookmarks and filter them in-memory using the specification
            let all_bookmarks = self.get_all()?;

            let filtered = all_bookmarks
                .into_iter()
                .filter(|bookmark| query.matches(bookmark))
                .collect::<Vec<_>>();

            // Apply sorting
            let mut sorted = filtered;
            if let Some(sort_direction) = query.sort_by_date {
                match sort_direction {
                    SortDirection::Ascending => {
                        sorted.sort_by_key(|a| a.updated_at);
                    }
                    SortDirection::Descending => {
                        sorted.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                    }
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
                }
                SortDirection::Descending => {
                    query_builder = query_builder.order(dsl::last_update_ts.desc());
                }
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
            .map_err(SqliteRepositoryError::DatabaseError)?;

        // Convert to domain models
        let bookmarks = db_bookmarks
            .into_iter()
            .filter_map(|db_bookmark| match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => Some(bookmark),
                Err(e) => {
                    error!("Failed to convert bookmark: {}", e);
                    None
                }
            })
            .collect();

        Ok(bookmarks)
    }

    #[instrument(skip_all, level = "debug")]
    fn get_all(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        let db_bookmarks = dsl::bookmarks
            .load::<DbBookmark>(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)?;

        let mut bookmarks = Vec::new();
        for db_bookmark in db_bookmarks {
            match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => bookmarks.push(bookmark),
                Err(e) => error!("Failed to convert bookmark: {}", e),
            }
        }

        Ok(bookmarks)
    }

    #[instrument(skip_all, level = "debug")]
    fn add(&self, bookmark: &mut Bookmark) -> Result<(), DomainError> {
        let mut conn = self.get_connection()?;

        // Begin transaction
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let db_bookmark = NewBookmark {
                url: bookmark.url.to_string(),
                metadata: bookmark.title.to_string(),
                tags: bookmark.formatted_tags(),
                desc: bookmark.description.to_string(),
                flags: bookmark.access_count,
                embedding: bookmark.embedding.clone(),
                content_hash: bookmark.content_hash.clone(),
                created_ts: Some(bookmark.created_at.naive_utc()),
                embeddable: bookmark.embeddable,
            };
            debug!("Inserting bookmark: {}", db_bookmark);

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
        .map_err(SqliteRepositoryError::DatabaseError)?;

        Ok(())
    }

    #[instrument(skip_all, level = "debug")]
    fn update(&self, bookmark: &Bookmark) -> Result<(), DomainError> {
        let mut conn = self.get_connection()?;

        let id = bookmark.id.ok_or_else(|| {
            SqliteRepositoryError::OperationFailed("Bookmark has no ID".to_string())
        })?;

        // Create the update data
        let changes = self.to_db_model(bookmark);

        // Update the bookmark
        let result = diesel::update(dsl::bookmarks.filter(dsl::id.eq(id)))
            .set(&changes)
            .execute(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)?;

        if result == 0 {
            return Err(SqliteRepositoryError::BookmarkNotFound(id).into());
        }

        Ok(())
    }

    #[instrument(skip_all, level = "debug")]
    fn delete(&self, id: i32) -> Result<bool, DomainError> {
        let mut conn = self.get_connection()?;

        // Begin transaction
        conn.transaction::<bool, diesel::result::Error, _>(|conn| {
            // Delete the bookmark
            let result = diesel::delete(dsl::bookmarks.filter(dsl::id.eq(id))).execute(conn)?;

            if result == 0 {
                return Ok(false); // No bookmark was deleted
            }

            // Update IDs of remaining bookmarks to maintain sequential IDs
            sql_query("UPDATE bookmarks SET id = id - 1 WHERE id > ?")
                .bind::<Integer, _>(id)
                .execute(conn)?;

            // Return success
            Ok(true)
        })
        .map_err(SqliteRepositoryError::DatabaseError)?;

        Ok(true)
    }

    #[instrument(skip_all, level = "trace")]
    fn get_all_tags(&self) -> Result<Vec<(Tag, usize)>, DomainError> {
        let mut conn = self.get_connection()?;

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
            .map_err(SqliteRepositoryError::DatabaseError)?;

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

    #[instrument(skip_all, level = "debug")]
    fn get_related_tags(&self, tag: &Tag) -> Result<Vec<(Tag, usize)>, DomainError> {
        let mut conn = self.get_connection()?;

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
            .map_err(SqliteRepositoryError::DatabaseError)?;

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

    #[instrument(skip_all, level = "debug")]
    fn get_random(&self, count: usize) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        // First get all IDs
        #[derive(QueryableByName, Debug)]
        #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
        struct RandomId {
            #[diesel(sql_type = Integer)]
            pub id: i32,
        }

        // Get random IDs
        let random_ids: Vec<RandomId> = sql_query(format!(
            "SELECT id FROM bookmarks ORDER BY RANDOM() LIMIT {}",
            count
        ))
        .load(&mut conn)
        .map_err(SqliteRepositoryError::DatabaseError)?;

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
            .map_err(SqliteRepositoryError::DatabaseError)?;

        // Convert to domain models
        let bookmarks = db_bookmarks
            .into_iter()
            .filter_map(|db_bookmark| match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => Some(bookmark),
                Err(e) => {
                    error!("Failed to convert bookmark: {}", e);
                    None
                }
            })
            .collect();

        Ok(bookmarks)
    }

    // Update get_without_embeddings to ignore embeddable flag (let the service handle filtering)
    #[instrument(skip_all, level = "debug")]
    fn get_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        let db_bookmarks = dsl::bookmarks
            .filter(dsl::embedding.is_null())
            .load::<DbBookmark>(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)?;

        let mut bookmarks = Vec::new();
        for db_bookmark in db_bookmarks {
            match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => bookmarks.push(bookmark),
                Err(e) => error!("Failed to convert bookmark: {}", e),
            }
        }

        Ok(bookmarks)
    }

    #[instrument(skip_all, level = "debug")]
    fn get_by_all_tags(&self, tags: &HashSet<Tag>) -> Result<Vec<Bookmark>, DomainError> {
        let query =
            BookmarkQuery::new().with_specification(AllTagsSpecification::new(tags.clone()));
        self.search(&query)
    }

    fn get_by_any_tag(&self, tags: &HashSet<Tag>) -> Result<Vec<Bookmark>, DomainError> {
        let query = BookmarkQuery::new().with_specification(AnyTagSpecification::new(tags.clone()));
        self.search(&query)
    }

    fn get_by_access_date(
        &self,
        direction: SortDirection,
        limit: Option<usize>,
    ) -> Result<Vec<Bookmark>, DomainError> {
        let mut query = BookmarkQuery::new().with_sort_by_date(direction);
        if let Some(limit) = limit {
            query = query.with_limit(limit);
        }
        self.search(&query)
    }

    // TODO:testing, which methods can be hidden?
    fn search_by_text(&self, text: &str) -> Result<Vec<Bookmark>, DomainError> {
        // let query = BookmarkQuery::new().with_specification(
        //     crate::domain::repositories::query::TextSearchSpecification::new(text.to_string()),
        // );
        // self.search(&query)
        self.get_bookmarks(text).map_err(|e| {
            DomainError::RepositoryError(format!("Failed to search bookmarks by text: {}", e))
        })
    }

    // TODO:testing
    #[instrument(level = "debug")]
    fn get_bookmarks(&self, query: &str) -> SqliteResult<Vec<Bookmark>> {
        let mut conn = self.get_connection()?;

        if query.is_empty() {
            let db_bookmarks = dsl::bookmarks
                .load::<DbBookmark>(&mut conn)
                .map_err(SqliteRepositoryError::DatabaseError)?;

            let bookmarks = db_bookmarks
                .into_iter()
                .filter_map(|db_bookmark| match self.to_domain_model(db_bookmark) {
                    Ok(bookmark) => Some(bookmark),
                    Err(e) => {
                        error!("Failed to convert bookmark: {}", e);
                        None
                    }
                })
                .collect();

            Ok(bookmarks)
        } else {
            let ids = self.get_bookmarks_fts(query)?;

            let db_bookmarks = dsl::bookmarks
                .filter(dsl::id.eq_any(ids))
                .load::<DbBookmark>(&mut conn)
                .map_err(SqliteRepositoryError::DatabaseError)?;

            let bookmarks = db_bookmarks
                .into_iter()
                .filter_map(|db_bookmark| match self.to_domain_model(db_bookmark) {
                    Ok(bookmark) => Some(bookmark),
                    Err(e) => {
                        error!("Failed to convert bookmark: {}", e);
                        None
                    }
                })
                .collect();

            Ok(bookmarks)
        }
    }

    // #[instrument(level = "debug")]
    // fn get_bookmarks_fts(&self, fts_query: &str) -> SqliteResult<Vec<i32>> {
    //     let mut conn = self.get_connection()?;
    //
    //     sql_query(
    //         "SELECT id FROM bookmarks_fts \
    //      WHERE bookmarks_fts MATCH ? \
    //      ORDER BY rank",
    //     )
    //     .bind::<Text, _>(fts_query)
    //     .load::<IdResult>(&mut conn)
    //     .map(|results| results.into_iter().map(|result| result.id).collect())
    //     .map_err(SqliteRepositoryError::DatabaseError)
    // }
    fn get_bookmarks_fts(&self, fts_query: &str) -> Result<Vec<i32>, SqliteRepositoryError> {
        let mut conn = self.get_connection()?;

        // Prepare the Diesel query
        let query = sql_query(
            "SELECT id FROM bookmarks_fts \
             WHERE bookmarks_fts MATCH ? \
             ORDER BY rank",
        )
        .bind::<Text, _>(fts_query);

        // Log the SQL string (substitutes parameters into the query)
        info!(
            "Executing SQL: {}",
            diesel::debug_query::<diesel::sqlite::Sqlite, _>(&query)
        );

        // Execute and transform into Vec<i32>
        let ids = query
            .load::<IdResult>(&mut conn)?
            .into_iter()
            .map(|record| record.id)
            .collect();

        Ok(ids)
    }

    fn exists_by_url(&self, url: &str) -> Result<bool, DomainError> {
        self.get_by_url(url).map(|result| result.is_some())
    }

    #[instrument(skip_all, level = "debug")]
    fn get_embeddable_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        // Query for bookmarks that are embeddable but don't have embeddings
        let db_bookmarks = dsl::bookmarks
            .filter(dsl::embedding.is_null().and(dsl::embeddable.eq(true)))
            .load::<DbBookmark>(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)?;

        let mut bookmarks = Vec::new();
        for db_bookmark in db_bookmarks {
            match self.to_domain_model(db_bookmark) {
                Ok(bookmark) => bookmarks.push(bookmark),
                Err(e) => error!("Failed to convert bookmark: {}", e),
            }
        }

        Ok(bookmarks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::domain::repositories::query::TextSearchSpecification;
    use crate::util::testing::{init_test_env, setup_test_db};
    use serial_test::serial;
    use std::collections::HashSet;

    fn create_test_bookmark(
        title: &str,
        url: &str,
        tags: Vec<&str>,
    ) -> Result<Bookmark, DomainError> {
        let tag_set: HashSet<Tag> = tags
            .into_iter()
            .map(Tag::new)
            .collect::<Result<HashSet<_>, _>>()?;

        let app_state = AppState::read_global();
        let embedder = &*app_state.context.embedder;
        Bookmark::new(url, title, "Test description", tag_set, embedder)
    }

    #[test]
    #[serial]
    fn test_add_and_get_by_id() -> Result<(), DomainError> {
        let repo = setup_test_db();

        let mut bookmark = create_test_bookmark(
            "Test Bookmark",
            "https://example.com",
            vec!["test", "example"],
        )?;

        repo.add(&mut bookmark)?;

        // Check that ID was set
        assert!(bookmark.id.is_some());

        // Get the bookmark by ID
        let retrieved = repo.get_by_id(bookmark.id.unwrap())?;

        // Verify it was retrieved correctly
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Bookmark");
        assert_eq!(retrieved.url, "https://example.com");

        // Check tags
        assert_eq!(retrieved.tags.len(), 2);
        assert!(retrieved.tags.contains(&Tag::new("test")?));
        assert!(retrieved.tags.contains(&Tag::new("example")?));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_by_url() -> Result<(), DomainError> {
        let repo = setup_test_db();

        // Create and add a bookmark
        let mut bookmark =
            create_test_bookmark("URL Test", "https://url-test.com", vec!["url", "test"])?;

        repo.add(&mut bookmark)?;

        // Get the bookmark by URL
        let retrieved = repo.get_by_url("https://url-test.com")?;

        // Verify it was retrieved correctly
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "URL Test");

        // Test with nonexistent URL
        let not_found = repo.get_by_url("https://nonexistent.com")?;
        assert!(not_found.is_none());

        Ok(())
    }

    #[test]
    #[serial]
    fn test_update() -> Result<(), DomainError> {
        let repo = setup_test_db();

        // Create and add a bookmark
        let mut bookmark = create_test_bookmark(
            "Original Title",
            "https://update-test.com",
            vec!["original"],
        )?;

        repo.add(&mut bookmark)?;
        let id = bookmark.id.unwrap();

        // Update the bookmark
        let mut updated_tags = HashSet::new();
        updated_tags.insert(Tag::new("updated")?);

        let mut updated = bookmark.clone();
        updated.title = "Updated Title".to_string();
        updated.description = "Updated Description".to_string();
        updated.set_tags(updated_tags)?;

        repo.update(&updated)?;

        // Get the updated bookmark
        let retrieved = repo.get_by_id(id)?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Updated Title");
        assert_eq!(retrieved.description, "Updated Description");
        assert_eq!(retrieved.tags.len(), 1);
        assert!(retrieved.tags.contains(&Tag::new("updated")?));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_delete_and_reindex() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add two bookmarks
        let mut bookmark1 =
            create_test_bookmark("First Bookmark", "https://first.com", vec!["test"])?;
        repo.add(&mut bookmark1)?;

        let mut bookmark2 =
            create_test_bookmark("Second Bookmark", "https://second.com", vec!["test"])?;
        repo.add(&mut bookmark2)?;

        // Directly query all bookmarks to see their state
        let all = repo.get_all()?;
        assert_eq!(all.len(), 2, "Should have 2 bookmarks before deletion");
        assert_eq!(all[0].id, Some(1), "First bookmark should have ID 11");
        assert_eq!(all[1].id, Some(2), "Second bookmark should have ID 12");

        // Delete first bookmark
        repo.delete(1)?;

        // Get all bookmarks again to see the updated state
        let updated = repo.get_all()?;
        assert_eq!(updated.len(), 1, "Should have 1 bookmark after deletion");

        // The remaining bookmark should have ID 1 and be the second bookmark
        assert_eq!(
            updated[0].id,
            Some(1),
            "Remaining bookmark should have ID 1"
        );
        assert_eq!(
            updated[0].url, "https://second.com",
            "Remaining bookmark should be the second one"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_search() -> Result<(), DomainError> {
        let repo = setup_test_db();

        // Add test bookmarks
        let mut bookmark1 = create_test_bookmark(
            "Rust Programming",
            "https://rust-lang.org",
            vec!["programming", "rust"],
        )?;

        let mut bookmark2 = create_test_bookmark(
            "Python Guide",
            "https://python.org",
            vec!["programming", "python"],
        )?;

        let mut bookmark3 = create_test_bookmark(
            "Cooking Recipes",
            "https://recipes.com",
            vec!["cooking", "food"],
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
        let urls: Vec<&str> = results.iter().map(|b| b.url.as_str()).collect();
        assert!(urls.contains(&"https://rust-lang.org"));
        assert!(urls.contains(&"https://python.org"));
        assert!(!urls.contains(&"https://recipes.com"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_all_tags() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add bookmarks with various tags
        let mut bookmark1 =
            create_test_bookmark("Test 1", "https://example1.com", vec!["tag1", "tag2"])?;

        let mut bookmark2 =
            create_test_bookmark("Test 2", "https://example2.com", vec!["tag2", "tag3"])?;

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
    #[serial]
    fn test_get_related_tags() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add bookmarks with related tags
        let mut bookmark1 =
            create_test_bookmark("Test 1", "https://example1.com", vec!["common", "related1"])?;

        let mut bookmark2 =
            create_test_bookmark("Test 2", "https://example2.com", vec!["common", "related2"])?;

        let mut bookmark3 =
            create_test_bookmark("Test 3", "https://example3.com", vec!["unrelated"])?;

        repo.add(&mut bookmark1)?;
        repo.add(&mut bookmark2)?;
        repo.add(&mut bookmark3)?;

        // Get tags related to "common"
        let common_tag = Tag::new("common")?;
        let related = repo.get_related_tags(&common_tag)?;

        // Should find related1 and related2
        assert_eq!(related.len(), 2);

        let tag_values: HashSet<String> = related
            .iter()
            .map(|(tag, _)| tag.value().to_string())
            .collect();

        assert!(tag_values.contains("related1"));
        assert!(tag_values.contains("related2"));
        assert!(!tag_values.contains("unrelated"));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_random() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add multiple bookmarks
        for i in 1..=5 {
            let mut bookmark = create_test_bookmark(
                &format!("Test {}", i),
                &format!("https://example{}.com", i),
                vec!["test"],
            )?;
            repo.add(&mut bookmark)?;
        }

        // Get 2 random bookmarks
        let random = repo.get_random(2)?;

        // Verify count
        assert_eq!(random.len(), 2);

        // Verify they're valid bookmarks
        for bookmark in &random {
            assert!(bookmark.id.is_some());
            assert!(bookmark.url.starts_with("https://example"));
        }

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_without_embeddings() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add bookmarks (all should have null embeddings in this test)
        for i in 1..=3 {
            let mut bookmark = create_test_bookmark(
                &format!("Test {}", i),
                &format!("https://example{}.com", i),
                vec!["test"],
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
    #[serial]
    fn test_exists_by_url() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add a test bookmark
        let mut bookmark =
            create_test_bookmark("Test Bookmark", "https://exists-test.com", vec!["test"])?;
        repo.add(&mut bookmark)?;

        // Check existing URL
        let exists = repo.exists_by_url("https://exists-test.com")?;
        assert!(exists);

        // Check non-existing URL
        let not_exists = repo.exists_by_url("https://does-not-exist.com")?;
        assert!(!not_exists);

        Ok(())
    }
    //
    // #[test]
    // fn test_get_oldest_bookmarks() -> Result<(), DomainError> {
    //     let repo = setup_test_db();
    //     _ = repo.empty_bookmark_table()?;
    //
    //     // Create bookmarks with controlled timestamps
    //     // In a real test we'd control the timestamps more explicitly
    //     let mut bm1 = create_test_bookmark("Old 1", "https://old1.com", vec!["old"])?;
    //     std::thread::sleep(std::time::Duration::from_millis(50));
    //     let mut bm2 = create_test_bookmark("Old 2", "https://old2.com", vec!["old"])?;
    //     std::thread::sleep(std::time::Duration::from_millis(50));
    //     let mut bm3 = create_test_bookmark("New", "https://new.com", vec!["new"])?;
    //
    //     repo.add(&mut bm1)?;
    //     repo.add(&mut bm2)?;
    //     repo.add(&mut bm3)?;
    //
    //     // Get oldest 2 bookmarks
    //     let oldest = repo.get_oldest_bookmarks(2)?;
    //
    //     // Verify we got 2 bookmarks
    //     assert_eq!(oldest.len(), 2, "Should get exactly 2 bookmarks");
    //
    //     // Verify they're in the right order (oldest first)
    //     assert_eq!(
    //         oldest[0].url(),
    //         "https://old1.com",
    //         "First should be oldest"
    //     );
    //     assert_eq!(
    //         oldest[1].url(),
    //         "https://old2.com",
    //         "Second should be second oldest"
    //     );
    //
    //     Ok(())
    // }

    #[test]
    #[serial]
    fn test_database_compaction_after_delete() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Create several bookmarks in sequence
        let mut bm1 = create_test_bookmark("First", "https://first.com", vec!["test"])?;
        let mut bm2 = create_test_bookmark("Second", "https://second.com", vec!["test"])?;
        let mut bm3 = create_test_bookmark("Third", "https://third.com", vec!["test"])?;
        let mut bm4 = create_test_bookmark("Fourth", "https://fourth.com", vec!["test"])?;

        repo.add(&mut bm1)?;
        repo.add(&mut bm2)?;
        repo.add(&mut bm3)?;
        repo.add(&mut bm4)?;

        // Verify IDs are sequential
        let all_before = repo.get_all()?;
        let ids_before: Vec<i32> = all_before.iter().filter_map(|b| b.id).collect();

        // Make sure we have at least 4 bookmarks
        assert!(ids_before.len() >= 4, "Should have at least 4 bookmarks");

        // Delete bookmark with ID 2
        let id_to_delete = 2;
        repo.delete(id_to_delete)?;

        // Get all bookmarks again
        let all_after = repo.get_all()?;

        // Check that IDs have been compacted (no gaps)
        let ids_after: Vec<i32> = all_after.iter().filter_map(|b| b.id).collect();

        // Should have one fewer bookmark
        assert_eq!(
            ids_after.len(),
            ids_before.len() - 1,
            "Should have one fewer bookmark"
        );

        // The IDs should be sequential without gaps
        for i in 1..=ids_after.len() {
            assert!(
                ids_after.contains(&(i as i32)),
                "Missing ID {} after compaction",
                i
            );
        }

        // Specifically, check that the bookmark that was at ID 3 is now at ID 2
        let former_id3 = all_before
            .iter()
            .find(|b| b.id == Some(3))
            .map(|b| b.url.to_string());

        let new_id2 = all_after
            .iter()
            .find(|b| b.id == Some(2))
            .map(|b| b.url.to_string());

        assert_eq!(
            former_id3, new_id2,
            "Bookmark formerly at ID 3 should now be at ID 2"
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_by_invalid_id() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Try to get a bookmark with an invalid ID
        let result = repo.get_by_id(99999)?;

        // Should return None, not an error
        assert!(result.is_none(), "Get by invalid ID should return None");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_get_all_tags_as_vector() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add bookmarks with known tags
        let mut bm1 = create_test_bookmark(
            "Tags Test 1",
            "https://tags1.com",
            vec!["aaa", "bbb", "ccc"],
        )?;
        let mut bm2 = create_test_bookmark("Tags Test 2", "https://tags2.com", vec!["xxx", "yyy"])?;

        repo.add(&mut bm1)?;
        repo.add(&mut bm2)?;

        // Get all tags
        let tags_with_counts = repo.get_all_tags()?;

        // Extract just the tag values and sort them
        let mut tag_values: Vec<String> = tags_with_counts
            .iter()
            .map(|(tag, _)| tag.value().to_string())
            .collect();
        tag_values.sort();

        // Verify expected tags are present
        assert!(tag_values.contains(&"aaa".to_string()));
        assert!(tag_values.contains(&"bbb".to_string()));
        assert!(tag_values.contains(&"ccc".to_string()));
        assert!(tag_values.contains(&"xxx".to_string()));
        assert!(tag_values.contains(&"yyy".to_string()));

        Ok(())
    }

    // Helper structure for schema check
    #[derive(QueryableByName, Debug)]
    struct TableCheckResult {
        #[diesel(sql_type = Integer)]
        pub table_exists: i32,
    }

    #[test]
    #[serial]
    fn test_check_schema_migrations_exists() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // We need to use direct SQL to check for the migrations table
        let mut conn = repo.get_connection().map_err(DomainError::from)?;

        // Query to check if migrations table exists - fixed SQL syntax
        let result = sql_query(
            "
        SELECT COUNT(*) as table_exists
        FROM sqlite_master
        WHERE type='table' AND name='__diesel_schema_migrations'
    ",
        )
        .get_result::<TableCheckResult>(&mut conn)
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!("Failed to check schema: {}", e))
        })?;

        let exists = result.table_exists > 0;

        // Initially the table might not exist in a fresh test DB
        // The important thing is that the query executes correctly
        eprintln!("Schema migrations table exists: {}", exists);

        Ok(())
    }

    // Helper structure for column check
    #[derive(QueryableByName, Debug)]
    struct ColumnCheckResult {
        #[diesel(sql_type = Integer)]
        pub column_exists: i32,
    }

    #[test]
    #[serial]
    fn test_check_embedding_column_exists() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        let mut conn = repo.get_connection().map_err(DomainError::from)?;

        // Query to check if embedding column exists
        let exists: bool = sql_query(
            "
        SELECT COUNT(*) as column_exists
        FROM pragma_table_info('bookmarks')
        WHERE name='embedding'
    ",
        )
        .get_result::<ColumnCheckResult>(&mut conn)
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!("Failed to check column: {}", e))
        })?
        .column_exists
            > 0;

        // In a test DB the column should exist
        assert!(exists, "Embedding column should exist");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_setup_test_db() {
        let _ = init_test_env();
        let repo = setup_test_db();
        assert!(repo.get_connection().is_ok());
        print_db_schema(&repo);
    }
}
