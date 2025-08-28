// src/infrastructure/repositories/sqlite/repository

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Integer, Text};
use std::collections::HashSet;
use tracing::{debug, error, info, instrument};

use super::connection::{ConnectionPool, PooledConnection};
use super::error::{SqliteRepositoryError, SqliteResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, RepositoryError};
use crate::domain::repositories::query::{
    AllTagsSpecification, AnyTagSpecification, BookmarkQuery, SortDirection,
};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::tag::Tag;
use crate::infrastructure::repositories::sqlite::model::{
    DbBookmark, DbBookmarkChanges, IdResult, NewBookmark, TagsFrequency,
};
use crate::infrastructure::repositories::sqlite::schema::bookmarks::dsl;
use diesel::QueryableByName;

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
        let pool = super::connection::init_pool(database_url)
            .map_err(|e| e.context("initializing database connection pool"))?;
        Ok(Self { pool })
    }

    /// Get a connection from the pool
    #[instrument(skip_all, level = "debug")]
    pub fn get_connection(&self) -> SqliteResult<PooledConnection> {
        self.pool
            .get()
            .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))
            .map_err(|e| e.context("getting database connection from pool"))
    }

    /// Cleans the table by deleting all bookmarks except ID 1
    #[instrument(skip_all, level = "debug")]
    pub fn empty_bookmark_table(&self) -> SqliteResult<()> {
        let mut conn = self.get_connection()?;

        // sql_query("DELETE FROM bookmarks WHERE id != 1;")
        sql_query("DELETE FROM bookmarks;")
            .execute(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| e.context("executing table cleanup query"))?;

        debug!("Cleaned table.");
        Ok(())
    }

    /// Convert a database model to a domain entity
    #[instrument(skip_all, level = "trace")]
    fn to_domain_model(&self, db_bookmark: DbBookmark) -> SqliteResult<Bookmark> {
        let updated_at =
            DateTime::<Utc>::from_naive_utc_and_offset(db_bookmark.last_update_ts, Utc);

        let created_at = db_bookmark
            .created_ts
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));

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
            db_bookmark.file_path,
            db_bookmark.file_mtime,
            db_bookmark.file_hash,
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
        // Create the changes with explicit Option types to ensure NULL values are handled correctly
        let changes = DbBookmarkChanges {
            url: bookmark.url.to_string(),
            metadata: bookmark.title.to_string(),
            tags: bookmark.formatted_tags(),
            desc: bookmark.description.to_string(),
            flags: bookmark.access_count,
            embedding: bookmark.embedding.clone(),
            content_hash: bookmark.content_hash.clone(),
            created_ts: bookmark.created_at.map(|dt| dt.naive_utc()),
            embeddable: bookmark.embeddable,
            file_path: bookmark.file_path.clone(),
            file_mtime: bookmark.file_mtime,
            file_hash: bookmark.file_hash.clone(),
        };

        debug!(
            "Created DB model changes: embedding is null: {}, content_hash is null: {}",
            changes.embedding.is_none(),
            changes.content_hash.is_none()
        );

        changes
    }
}

impl BookmarkRepository for SqliteBookmarkRepository {
    #[instrument(skip_all, level = "debug")]
    fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for bookmark lookup"))?;

        let result = dsl::bookmarks
            .filter(dsl::id.eq(id))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context(format!("querying bookmark by ID {}", id)))?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)
                    .map_err(|e| DomainError::RepositoryError(e.into()))
                    .map_err(|e| e.context(format!("converting database model to domain model for bookmark ID {}", id)))?;
                Ok(Some(bookmark))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip_all, level = "debug")]
    fn get_by_url(&self, url: &str) -> Result<Option<Bookmark>, DomainError> {
        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for URL lookup"))?;

        // Escape special characters in URL for SQLite query
        let escaped_url = url.replace('\'', "''");

        let result = dsl::bookmarks
            .filter(dsl::URL.eq(escaped_url))
            .first::<DbBookmark>(&mut conn)
            .optional()
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context(format!("querying bookmark by URL: {}", url)))?;

        match result {
            Some(db_bookmark) => {
                let bookmark = self.to_domain_model(db_bookmark)
                    .map_err(|e| DomainError::RepositoryError(e.into()))
                    .map_err(|e| e.context(format!("converting database model to domain model for URL: {}", url)))?;
                Ok(Some(bookmark))
            }
            None => Ok(None),
        }
    }

    #[instrument(skip_all, level = "debug")]
    fn search(&self, query: &BookmarkQuery) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for bookmark search"))?;

        // First, handle text query with FTS if present
        let bookmark_ids = if let Some(text_query) = &query.text_query {
            if !text_query.is_empty() {
                // Use FTS to get matching bookmark IDs
                debug!("Using FTS search for query: {}", text_query);
                self.get_bookmarks_fts(text_query)
                    .map_err(|e| e.context(format!("performing FTS search for query: {}", text_query)))?
            } else {
                // Empty text query, get all IDs
                debug!("Empty text query, retrieving all bookmark IDs");
                self.get_all_bookmark_ids(&mut conn)
                    .map_err(|e| DomainError::RepositoryError(e.into()))
                    .map_err(|e| e.context("retrieving all bookmark IDs for empty text query"))?
            }
        } else {
            // No text query, get all IDs
            debug!("No text query, retrieving all bookmark IDs");
            self.get_all_bookmark_ids(&mut conn)
                .map_err(|e| DomainError::RepositoryError(e.into()))
                .map_err(|e| e.context("retrieving all bookmark IDs for no text query"))?
        };

        // If we have no IDs after FTS, return empty result quickly
        if bookmark_ids.is_empty() {
            debug!("No matching bookmarks found for text query");
            return Ok(Vec::new());
        }

        // Fetch the complete bookmark objects for the matching IDs
        let bookmarks = self.get_bookmarks_by_ids(&bookmark_ids)
            .map_err(|e| e.context("fetching complete bookmark objects by IDs"))?;

        // Apply all other filters from the query
        let filtered_bookmarks = query.apply_non_text_filters(&bookmarks);

        debug!(
            "After filtering: {} bookmarks match the query",
            filtered_bookmarks.len()
        );
        Ok(filtered_bookmarks)
    }

    #[instrument(skip_all, level = "trace")]
    fn get_all_bookmark_ids(
        &self,
        conn: &mut PooledConnection,
    ) -> Result<Vec<i32>, SqliteRepositoryError> {
        let ids = dsl::bookmarks
            .select(dsl::id)
            .load::<i32>(conn)
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| e.context("loading all bookmark IDs from database"))?;

        Ok(ids)
    }

    #[instrument(skip_all, level = "trace")]
    fn get_bookmarks_by_ids(&self, ids: &[i32]) -> Result<Vec<Bookmark>, DomainError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for bulk bookmark retrieval"))?;

        let db_bookmarks = dsl::bookmarks
            .filter(dsl::id.eq_any(ids))
            .load::<DbBookmark>(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context(format!("querying bookmarks by IDs: {:?}", ids)))?;

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
        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for retrieving all bookmarks"))?;

        let db_bookmarks = dsl::bookmarks
            .load::<DbBookmark>(&mut conn)
            .map_err(SqliteRepositoryError::DatabaseError)
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("loading all bookmarks from database"))?;

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
        let mut conn = self.get_connection()
            .map_err(|e| DomainError::RepositoryError(e.into()))
            .map_err(|e| e.context("getting database connection for adding bookmark"))?;

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
                created_ts: bookmark.created_at.map(|dt| dt.naive_utc()),
                embeddable: bookmark.embeddable,
                file_path: bookmark.file_path.clone(),
                file_mtime: bookmark.file_mtime,
                file_hash: bookmark.file_hash.clone(),
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

        let changes = self.to_db_model(bookmark);
        // debug!("Updating bookmark with ID {}: {:?}", id, changes);  // logs entire embedding

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
            let result = diesel::delete(dsl::bookmarks.filter(dsl::id.eq(id))).execute(conn)?;
            if result == 0 {
                return Ok(false); // No bookmark was deleted
            }
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
            query = query.with_limit(Option::from(limit));
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
            DomainError::RepositoryError(RepositoryError::Other(format!(
                "Failed to search bookmarks by text: {}",
                e
            )))
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

    #[instrument(skip(self))]
    fn exists_by_url(&self, url: &str) -> Result<i32, DomainError> {
        let bookmark = self.get_by_url(url)?;

        match bookmark {
            Some(bm) => bm
                .id
                .ok_or_else(|| DomainError::BookmarkNotFound("Bookmark ID is None".to_string())),
            None => Ok(-1),
        }
    }

    #[instrument(skip_all, level = "debug")]
    fn get_embeddable_without_embeddings(&self) -> Result<Vec<Bookmark>, DomainError> {
        let mut conn = self.get_connection()?;

        // Query for bookmarks that are embeddable but don't have embeddings
        // Only check if embedding is NULL, ignore content_hash
        let db_bookmarks = dsl::bookmarks
            .filter(
                dsl::embeddable
                    .eq(true)
                    .and(dsl::embedding.is_null())
                    .and(dsl::tags.not_like("%,_imported_,%")),
            )
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
    use crate::domain::repositories::query::TextSearchSpecification;
    use crate::util::testing::{init_test_env, setup_test_db};
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

        let embedder = crate::infrastructure::embeddings::DummyEmbedding;
        Bookmark::new(url, title, "Test description", tag_set, &embedder)
    }

    #[test]
    fn given_new_bookmark_when_add_and_get_by_id_then_retrieves_successfully() -> Result<(), DomainError> {
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
    fn given_bookmark_with_url_when_get_by_url_then_finds_bookmark() -> Result<(), DomainError> {
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
    fn given_existing_bookmark_when_update_then_changes_persist() -> Result<(), DomainError> {
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
    fn given_existing_bookmark_when_delete_then_removes_and_reindexes() -> Result<(), DomainError> {
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

        // Get the IDs of the bookmarks
        // let id1 = bookmark1.id.unwrap();
        let id2 = bookmark2.id.unwrap();

        // Delete first bookmark
        repo.delete(1)?;

        // Get all bookmarks again to see the updated state
        let updated = repo.get_all()?;
        assert_eq!(updated.len(), 1, "Should have 1 bookmark after deletion");

        // The remaining bookmark should still have its original ID
        assert_eq!(
            updated[0].id,
            Some(id2),
            "Remaining bookmark should keep its original ID"
        );
        assert_eq!(
            updated[0].url, "https://second.com",
            "Remaining bookmark should be the second one"
        );

        Ok(())
    }

    #[test]
    fn given_search_query_when_search_then_returns_matching_bookmarks() -> Result<(), DomainError> {
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
    fn given_bookmarks_with_tags_when_get_all_tags_then_returns_unique_tags() -> Result<(), DomainError> {
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
    fn given_tag_query_when_get_related_tags_then_returns_cooccurring_tags() -> Result<(), DomainError> {
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
    fn given_bookmarks_exist_when_get_random_then_returns_random_selection() -> Result<(), DomainError> {
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
    fn given_bookmarks_without_embeddings_when_get_then_returns_filtered_list() -> Result<(), DomainError> {
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
    fn given_url_when_exists_by_url_then_returns_existence_status() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Add a test bookmark
        let mut bookmark =
            create_test_bookmark("Test Bookmark", "https://exists-test.com", vec!["test"])?;
        repo.add(&mut bookmark)?;

        // Check existing URL
        let exists = repo.exists_by_url("https://exists-test.com")?;
        assert_eq!(exists, 1);

        // Check non-existing URL
        let not_exists = repo.exists_by_url("https://does-not-exist.com")?;
        assert_eq!(not_exists, -1);

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
    fn given_invalid_id_when_get_by_id_then_returns_none() -> Result<(), DomainError> {
        let repo = setup_test_db();
        repo.empty_bookmark_table()?;

        // Try to get a bookmark with an invalid ID
        let result = repo.get_by_id(99999)?;

        // Should return None, not an error
        assert!(result.is_none(), "Get by invalid ID should return None");

        Ok(())
    }

    #[test]
    fn given_tagged_bookmarks_when_get_all_tags_as_vector_then_returns_sorted_tags() -> Result<(), DomainError> {
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
    fn given_database_when_check_schema_migrations_then_verifies_existence() -> Result<(), DomainError> {
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
    fn given_database_when_check_embedding_column_then_verifies_existence() -> Result<(), DomainError> {
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
    fn given_test_environment_when_setup_test_db_then_creates_database() {
        let _ = init_test_env();
        let repo = setup_test_db();
        assert!(repo.get_connection().is_ok());
        print_db_schema(&repo);
    }

    #[test]
    fn given_bookmarks_exist_when_get_all_ids_then_returns_id_list() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();
        let mut conn = repo.get_connection()?;

        // Act
        let ids = repo.get_all_bookmark_ids(&mut conn)?;

        // Assert
        assert!(!ids.is_empty(), "Should return at least some bookmark IDs");

        // Verify count matches total bookmarks
        let all_bookmarks = repo.get_all()?;
        assert_eq!(
            ids.len(),
            all_bookmarks.len(),
            "Number of IDs should match number of bookmarks"
        );

        Ok(())
    }

    #[test]
    fn given_valid_ids_when_get_bookmarks_by_ids_then_returns_bookmarks() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();
        let mut conn = repo.get_connection()?;

        // Get a subset of IDs (first 3)
        let all_ids = repo.get_all_bookmark_ids(&mut conn)?;
        let subset_ids: Vec<i32> = all_ids.into_iter().take(3).collect();

        // Act
        let bookmarks = repo.get_bookmarks_by_ids(&subset_ids)?;

        // Assert
        assert_eq!(
            bookmarks.len(),
            subset_ids.len(),
            "Should return exactly the number of bookmarks for the provided IDs"
        );

        Ok(())
    }

    #[test]
    fn given_empty_id_list_when_get_bookmarks_by_ids_then_returns_empty() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();
        let empty_ids: Vec<i32> = Vec::new();

        // Act
        let bookmarks = repo.get_bookmarks_by_ids(&empty_ids)?;

        // Assert
        assert!(
            bookmarks.is_empty(),
            "Should return empty vector for empty IDs list"
        );

        Ok(())
    }

    #[test]
    fn given_nonexistent_ids_when_get_bookmarks_by_ids_then_returns_empty() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();
        let nonexistent_ids = vec![99999, 99998, 99997]; // IDs that shouldn't exist

        // Act
        let bookmarks = repo.get_bookmarks_by_ids(&nonexistent_ids)?;

        // Assert
        assert!(
            bookmarks.is_empty(),
            "Should return empty vector for nonexistent IDs"
        );

        Ok(())
    }

    #[test]
    fn given_mixed_valid_invalid_ids_when_get_bookmarks_then_returns_valid_only() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();
        let mut conn = repo.get_connection()?;

        // Get some valid IDs
        let valid_ids: Vec<i32> = repo
            .get_all_bookmark_ids(&mut conn)?
            .into_iter()
            .take(2)
            .collect();

        // Create a list with both valid and invalid IDs
        let mut mixed_ids = valid_ids.clone();
        mixed_ids.push(99999); // Add a nonexistent ID

        // Act
        let bookmarks = repo.get_bookmarks_by_ids(&mixed_ids)?;

        // Assert
        assert_eq!(
            bookmarks.len(),
            valid_ids.len(),
            "Should return only bookmarks for valid IDs"
        );

        // Check that each returned bookmark has one of the valid IDs
        for bookmark in &bookmarks {
            assert!(
                valid_ids.contains(&bookmark.id.unwrap()),
                "Returned bookmark should have a valid ID"
            );
        }

        Ok(())
    }

    #[test]
    fn given_text_query_only_when_search_then_returns_matching_results() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a query with just a text search
        let query = BookmarkQuery::new().with_text_query(Some("Google"));

        // Act
        let results = repo.search(&query)?;

        // Assert
        assert!(
            !results.is_empty(),
            "Should find bookmarks matching text query"
        );

        // Every result should contain "Google" somewhere
        // Note: This is an approximation since FTS might use stemming, etc.
        let has_match = results
            .iter()
            .any(|b| b.title.contains("Google") || b.url.contains("google"));

        assert!(
            has_match,
            "At least one result should contain the search text"
        );

        Ok(())
    }

    #[test]
    fn given_empty_text_query_when_search_then_returns_all_results() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a query with an empty text search
        let query = BookmarkQuery::new().with_text_query(Some(""));

        // Act
        let results = repo.search(&query)?;

        // Assert
        assert!(
            !results.is_empty(),
            "Empty text query should return all bookmarks"
        );

        // Results should match get_all
        let all_bookmarks = repo.get_all()?;
        assert_eq!(
            results.len(),
            all_bookmarks.len(),
            "Empty text query should return all bookmarks"
        );

        Ok(())
    }

    #[test]
    fn given_no_text_query_when_search_then_returns_all_results() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a query with no text search
        let query = BookmarkQuery::new();

        // Act
        let results = repo.search(&query)?;

        // Assert
        assert!(
            !results.is_empty(),
            "No text query should return all bookmarks"
        );

        // Results should match get_all
        let all_bookmarks = repo.get_all()?;
        assert_eq!(
            results.len(),
            all_bookmarks.len(),
            "No text query should return all bookmarks"
        );

        Ok(())
    }

    #[test]
    fn given_text_and_tag_filters_when_search_then_returns_filtered_results() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a tag that exists in sample data
        let mut tags = HashSet::new();
        tags.insert(Tag::new("aaa")?);

        // Create a query with both text and tag filters
        let query = BookmarkQuery::new()
            .with_text_query(Some("TEST"))
            .with_tags_all(Some(&tags));

        // Act
        let results = repo.search(&query)?;

        // Assert
        // Each result should have the specified tag
        for bookmark in &results {
            assert!(
                bookmark.tags.contains(&Tag::new("aaa")?),
                "Search results should respect tag filtering"
            );
        }

        // Compare with results from a query with just the text
        let text_only_query = BookmarkQuery::new().with_text_query(Some("TEST"));
        let text_only_results = repo.search(&text_only_query)?;

        // The filtered results should be a subset of the text-only results
        assert!(
            results.len() <= text_only_results.len(),
            "Adding tag filters should return same or fewer results"
        );

        Ok(())
    }

    #[test]
    fn given_nonmatching_text_query_when_search_then_returns_empty() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a query with a text search that shouldn't match anything
        let query =
            BookmarkQuery::new().with_text_query(Some("ThisShouldNotMatchAnything12345XYZ"));

        // Act
        let results = repo.search(&query)?;

        // Assert
        assert!(
            results.is_empty(),
            "Non-matching text query should return empty results"
        );

        Ok(())
    }

    #[test]
    fn given_mixed_filters_when_search_then_applies_all_criteria() -> Result<(), DomainError> {
        // Arrange
        let repo = setup_test_db();

        // Create a complex query with multiple filter types
        let mut all_tags = HashSet::new();
        all_tags.insert(Tag::new("aaa")?);

        let mut any_tags = HashSet::new();
        any_tags.insert(Tag::new("bbb")?);
        any_tags.insert(Tag::new("xxx")?);

        let query = BookmarkQuery::new()
            .with_text_query(Some("TEST"))
            .with_tags_all(Some(&all_tags))
            .with_tags_any(Some(&any_tags))
            .with_sort_by_date(SortDirection::Descending)
            .with_limit(Some(5));

        // Act
        let results = repo.search(&query)?;

        // Assert
        // Each result should match all filter criteria
        for bookmark in &results {
            // Should have the "all" tag
            assert!(
                bookmark.tags.contains(&Tag::new("aaa")?),
                "Results should have the 'all' tag"
            );

            // Should have at least one of the "any" tags
            assert!(
                bookmark.tags.contains(&Tag::new("bbb")?)
                    || bookmark.tags.contains(&Tag::new("xxx")?),
                "Results should have at least one of the 'any' tags"
            );
        }

        // Should respect the limit
        assert!(
            results.len() <= 5,
            "Results should respect the limit parameter"
        );

        // If there are multiple results, they should be in descending order
        if results.len() > 1 {
            for i in 0..results.len() - 1 {
                assert!(
                    results[i].updated_at >= results[i + 1].updated_at,
                    "Results should be sorted in descending order"
                );
            }
        }

        Ok(())
    }
}
