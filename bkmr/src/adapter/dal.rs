use std::fmt;
use std::fmt::Debug;

use anyhow::{Context, Result};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{Integer, Text};
use diesel::{sql_query, Connection, RunQueryDsl, SqliteConnection};
use tracing::{debug, instrument, trace};
use schema::bookmarks::dsl::bookmarks;
use schema::bookmarks::{
    content_hash, desc, embedding, flags, id, metadata, tags, URL,
};
use crate::model::bookmark::{Bookmark, IdResult, NewBookmark, TagsFrequency};

pub mod schema;
pub mod migration;

// trait DalTrait {
//     fn delete_bookmark(&mut self, id_: i32) -> Result<Vec<Bookmark>>;
//     fn batch_execute(&mut self, id_: i32) -> Result<()>;
//     fn delete_bookmark2(&mut self, id_: i32) -> Result<usize>;
//     fn clean_table(&mut self) -> Result<()>;
//     fn update_bookmark(&mut self, bm: Bookmark) -> Result<Vec<Bookmark>>;
//     fn insert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>>;
//     fn upsert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>>;
//     fn get_bookmark_by_id(&mut self, id_: i32) -> Result<Bookmark>;
//     fn get_bookmark_by_url(&mut self, url: &str) -> Result<Bookmark>;
//     fn get_bookmarks(&mut self, query: &str) -> Result<Vec<Bookmark>>;
//     fn get_bookmarks_fts(&mut self, fts_query: &str) -> Result<Vec<i32>>;
//     fn get_bookmarks_without_embedding(&mut self) -> Result<Vec<Bookmark>>;
//     fn bm_exists(&mut self, url: &str) -> Result<bool>;
//     fn get_all_tags(&mut self) -> Result<Vec<TagsFrequency>>;
//     fn get_all_tags_as_vec(&mut self) -> Result<Vec<String>>;
//     fn get_related_tags(&mut self, tag: &str) -> Result<Vec<TagsFrequency>>;
//     fn get_randomized_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>>;
//     fn get_oldest_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>>;
//     fn check_schema_migrations_exists(&mut self) -> Result<bool>;
//     fn check_embedding_column_exists(&mut self) -> Result<bool>;
// }

pub struct Dal {
    url: String,
    pub conn: SqliteConnection,
}

impl Dal {
    pub fn new(url: String) -> Self {
        debug!("{:?}", url);
        Self {
            conn: Dal::establish_connection(&url),
            url,
        }
    }

    fn establish_connection(database_url: &str) -> SqliteConnection {
        SqliteConnection::establish(database_url)
            .unwrap_or_else(|e| panic!("Error connecting to {}: {:?}", database_url, e))
    }

    #[instrument(level = "debug")]
    pub fn delete_bookmark(&mut self, id_: i32) -> Result<Vec<Bookmark>> {
        diesel::delete(bookmarks.filter(id.eq(id_)))
            .get_results(&mut self.conn)
            .with_context(|| format!("Failed to delete bookmark with id {}", id_))
    }

    #[instrument(level = "debug")]
    pub fn batch_execute(&mut self, id_: i32) -> Result<()> {
        let query = "
            BEGIN TRANSACTION;
            DELETE FROM bookmarks WHERE id = 2;
            UPDATE bookmarks SET id = id - 1 WHERE id > 2;
            COMMIT;
        ";
        self.conn
            .batch_execute(query)
            .with_context(|| format!("Failed to execute batch operation for id {}", id_))?;
        debug!("Deleted and Compacted {:?}",id_);
        Ok(())
    }

    #[instrument(level = "debug")]
    pub fn delete_bookmark2(&mut self, id_: i32) -> Result<usize> {
        sql_query("BEGIN TRANSACTION;")
            .execute(&mut self.conn)
            .with_context(|| "Failed to begin transaction")?;

        // Gotcha: 'returning *' not working within transaction
        let n = sql_query(
            "
            DELETE FROM bookmarks
            WHERE id = ?;
        ",
        )
        .bind::<Integer, _>(id_)
        .execute(&mut self.conn)
        .with_context(|| format!("Failed to delete bookmark with id {}", id_))?;

        // database compaction
        sql_query(
            "
            UPDATE bookmarks
            SET id = id - 1
            WHERE id > ?;
        ",
        )
        .bind::<Integer, _>(id_)
        .execute(&mut self.conn)
        .with_context(|| "Failed to compact bookmarks table")?;

        sql_query("COMMIT;")
            .execute(&mut self.conn)
            .with_context(|| "Failed to commit transaction")?;

        debug!("Deleted and Compacted, n: {:?}",n);
        Ok(n)
    }

    #[instrument(level = "debug")]
    pub fn clean_table(&mut self) -> Result<()> {
        sql_query("DELETE FROM bookmarks WHERE id != 1;")
            .execute(&mut self.conn)
            .with_context(|| "Failed to clean table")?;
        debug!("{:?}", "Cleaned table.");
        Ok(())
    }

    #[instrument(level = "debug")]
    pub fn update_bookmark(&mut self, bm: Bookmark) -> Result<Vec<Bookmark>> {
        diesel::update(bookmarks.find(bm.id))
            .set((
                URL.eq(bm.URL),
                metadata.eq(bm.metadata),
                tags.eq(bm.tags),
                desc.eq(bm.desc),
                flags.eq(bm.flags),
                embedding.eq(bm.embedding),
                content_hash.eq(bm.content_hash),
            ))
            .get_results(&mut self.conn)
            .with_context(|| format!("Failed to update bookmark with id {}", bm.id))
    }

    #[instrument(level = "debug")]
    pub fn insert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>> {
        diesel::insert_into(bookmarks)
            .values(bm)
            .get_results(&mut self.conn)
            .with_context(|| "Failed to insert bookmark")
    }

    #[instrument(level = "debug")]
    pub fn upsert_bookmark(&mut self, new_bm: NewBookmark) -> Result<Vec<Bookmark>> {
        match self.get_bookmark_by_url(&new_bm.URL) {
            Ok(bm) => self.update_bookmark(Bookmark {
                id: bm.id,
                URL: bm.URL.clone(),
                metadata: new_bm.metadata.clone(),
                tags: bm.tags.clone(),
                desc: bm.desc.clone(),
                flags: bm.flags,
                last_update_ts: chrono::Utc::now().naive_utc(),
                embedding: new_bm.embedding.clone(),
                content_hash: new_bm.content_hash.clone(),
            }),
            Err(_) => self.insert_bookmark(new_bm),
        }
    }

    #[instrument(level = "debug")]
    pub fn get_bookmark_by_id(&mut self, id_: i32) -> Result<Bookmark> {
        sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash FROM bookmarks \
        where id = ?;",
        )
            .bind::<Integer, _>(id_)
            .get_result(&mut self.conn)
            .map_err(|e| match e {
                DieselError::NotFound => anyhow::anyhow!("Bookmark with id {} not found", id_),
                e => anyhow::anyhow!("Database error while fetching bookmark {}: {}", id_, e)
            })
    }

    // In dal.rs
    #[instrument(level = "debug")]
    pub fn get_bookmark_by_url(&mut self, url: &str) -> Result<Bookmark> {
        // Escape special characters in URL for SQLite query
        let escaped_url = url.replace('\'', "''");

        sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash
         FROM bookmarks
         WHERE URL = ?;",
        )
        .bind::<Text, _>(&escaped_url)
        .get_result(&mut self.conn)
        .map_err(|e| match e {
            DieselError::NotFound => e.into(), // Preserve the original diesel error
            e => anyhow::anyhow!("Database error while fetching bookmark {}: {}", url, e),
        })
    }

    #[instrument(level = "debug")]
    pub fn get_bookmarks(&mut self, query: &str) -> Result<Vec<Bookmark>> {
        if query.is_empty() {
            bookmarks
                .load::<Bookmark>(&mut self.conn)
                .with_context(|| "Failed to load all bookmarks")
        } else {
            let ids = self.get_bookmarks_fts(query)?;
            bookmarks
                .filter(id.eq_any(ids))
                .load::<Bookmark>(&mut self.conn)
                .with_context(|| format!("Failed to load bookmarks matching query '{}'", query))
        }
    }

    #[instrument(level = "debug")]
    pub fn get_bookmarks_fts(&mut self, fts_query: &str) -> Result<Vec<i32>> {
        sql_query(
            "SELECT id FROM bookmarks_fts \
            WHERE bookmarks_fts MATCH ? \
            ORDER BY rank",
        )
        .bind::<Text, _>(fts_query)
        .load::<IdResult>(&mut self.conn)
        .map(|results| results.into_iter().map(|result| result.id).collect())
        .with_context(|| {
            format!(
                "Failed to perform full-text search with query '{}'",
                fts_query
            )
        })
    }

    #[instrument(level = "debug")]
    pub fn get_bookmarks_without_embedding(&mut self) -> Result<Vec<Bookmark>> {
        bookmarks
            .filter(embedding.is_null())
            .load::<Bookmark>(&mut self.conn)
            .with_context(|| "Failed to get bookmarks without embedding")
    }

    pub fn bm_exists(&mut self, url: &str) -> Result<bool> {
        sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash FROM bookmarks \
            where URL = ?;",
        )
            .bind::<Text, _>(url)
            .get_results::<Bookmark>(&mut self.conn)
            .map(|bms| !bms.is_empty())
            .with_context(|| format!("Failed to check existence of bookmark with URL {}", url))
    }

    /// get frequency based ordered list of all tags
    #[instrument(level = "debug")]
    pub fn get_all_tags(&mut self) -> Result<Vec<TagsFrequency>> {
        sql_query(
            "
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
        ",
        )
        .get_results(&mut self.conn)
        .with_context(|| "Failed to get all tags")
    }

    #[instrument(level = "debug")]
    pub fn get_all_tags_as_vec(&mut self) -> Result<Vec<String>> {
        let all_tags = self.get_all_tags()?;
        let mut all_tags: Vec<String> = all_tags.into_iter().map(|t| t.tag).collect();
        debug!("{:?}", all_tags);
        all_tags.sort();
        Ok(all_tags)
    }

    #[instrument(level = "debug")]
    pub fn get_related_tags(&mut self, tag: &str) -> Result<Vec<TagsFrequency>> {
        let search_tag = format!("%,{},%", tag);
        sql_query(
            "
            WITH RECURSIVE split(tags, rest) AS (
                SELECT '', tags || ','
                FROM bookmarks
                WHERE tags LIKE :tag_query
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
        ",
        )
        .bind::<Text, _>(search_tag)
        .get_results(&mut self.conn)
        .with_context(|| format!("Failed to get related tags for tag '{}'", tag))
    }

    #[instrument(level = "debug")]
    pub fn get_randomized_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>> {
        sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY RANDOM()
            LIMIT ?;",
        )
        .bind::<Integer, _>(n)
        .get_results(&mut self.conn)
        .with_context(|| format!("Failed to get {} random bookmarks", n))
    }

    #[instrument(level = "debug")]
    pub fn get_oldest_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>> {
        sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY last_update_ts ASC
            LIMIT ?;",
        )
        .bind::<Integer, _>(n)
        .get_results(&mut self.conn)
        .with_context(|| format!("Failed to get {} oldest bookmarks", n))
    }

    #[instrument(level = "trace")]
    pub fn check_schema_migrations_exists(&mut self) -> Result<bool> {
        let query = "
            SELECT 1 as diesel_exists FROM sqlite_master WHERE type='table' AND name='__diesel_schema_migrations';
        ";

        let result: Vec<ExistenceCheck> = sql_query(query)
            .load(&mut self.conn)
            .with_context(|| "Failed to check schema migrations existence")?;

        trace!("ExistenceCheck: {:?}", result);
        Ok(!result.is_empty())
    }

    #[instrument(level = "trace")]
    pub fn check_embedding_column_exists(&mut self) -> Result<bool> {
        let query = "
        SELECT COUNT(*) as column_exists
        FROM pragma_table_info('bookmarks')
        WHERE name='embedding';
        ";

        let result: Vec<ColumnCheck> = sql_query(query)
            .load(&mut self.conn)
            .with_context(|| "Failed to check embedding column existence")?;

        trace!("Embedding ColumnCheck: {:?}", result);
        Ok(result.iter().any(|item| item.column_exists > 0))
    }
}

#[derive(QueryableByName, Debug)]
struct ExistenceCheck {
    #[allow(dead_code)]
    #[diesel(sql_type = Integer)]
    diesel_exists: i32,
}

#[derive(QueryableByName, Debug)]
struct ColumnCheck {
    #[diesel(sql_type = Integer)]
    column_exists: i32,
}

impl Debug for Dal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.url)
    }
}
