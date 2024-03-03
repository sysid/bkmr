use std::fmt;
use std::fmt::Debug;

use anyhow::Context;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{Integer, Text};
use diesel::{sql_query, Connection, RunQueryDsl, SqliteConnection};
use log::debug;
use stdext::function_name;

use crate::adapter::schema::bookmarks::dsl::bookmarks;
use crate::adapter::schema::bookmarks::{
    content_hash, desc, embedding, flags, id, metadata, tags, URL,
};
use crate::dlog2;
use crate::model::bookmark::{Bookmark, IdResult, NewBookmark, TagsFrequency};

trait DalTrait {
    fn delete_bookmark(&mut self, id_: i32) -> Result<Vec<Bookmark>, DieselError>;
    fn batch_execute(&mut self, id_: i32) -> Result<(), DieselError>;
    fn delete_bookmark2(&mut self, id_: i32) -> Result<usize, DieselError>;
    fn clean_table(&mut self) -> Result<(), DieselError>;
    fn update_bookmark(&mut self, bm: Bookmark) -> Result<Vec<Bookmark>, DieselError>;
    fn insert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>, DieselError>;
    fn upsert_bookmark(&mut self, new_bm: NewBookmark) -> Result<Vec<Bookmark>, DieselError>;
    fn get_bookmark_by_id(&mut self, id_: i32) -> Result<Bookmark, DieselError>;
    fn get_bookmark_by_url(&mut self, url: &str) -> Result<Bookmark, DieselError>;
    fn get_bookmarks(&mut self, query: &str) -> Result<Vec<Bookmark>, DieselError>;
    fn get_bookmarks_fts(&mut self, fts_query: &str) -> Result<Vec<i32>, DieselError>;
    fn get_bookmarks_without_embedding(&mut self) -> Result<Vec<Bookmark>, DieselError>;
    fn bm_exists(&mut self, url: &str) -> Result<bool, DieselError>;
    fn get_all_tags(&mut self) -> Result<Vec<TagsFrequency>, DieselError>;
    fn get_all_tags_as_vec(&mut self) -> Result<Vec<String>, anyhow::Error>;
    fn get_related_tags(&mut self, tag: &str) -> Result<Vec<TagsFrequency>, DieselError>;
    fn get_randomized_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError>;
    fn get_oldest_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError>;
    fn check_schema_migrations_exists(&mut self) -> Result<bool, DieselError>;
    fn check_embedding_column_exists(&mut self) -> Result<bool, DieselError>;
}

pub struct Dal {
    // #[allow(dead_code)]
    url: String,
    pub conn: SqliteConnection,
}

impl Dal {
    pub fn new(url: String) -> Self {
        debug!("({}:{}) {:?}", function_name!(), line!(), url);
        Self {
            conn: Dal::establish_connection(&url),
            url,
        }
    }

    fn establish_connection(database_url: &str) -> SqliteConnection {
        SqliteConnection::establish(database_url)
            .unwrap_or_else(|e| panic!("Error connecting to {}: {:?}", database_url, e))
    }

    pub fn delete_bookmark(&mut self, id_: i32) -> Result<Vec<Bookmark>, DieselError> {
        // diesel::delete(bookmarks.filter(id.eq(1))).execute(&mut self.conn)
        diesel::delete(bookmarks.filter(id.eq(id_))).get_results(&mut self.conn)
    }
    /// POC for multiple statements, not used in application
    pub fn batch_execute(&mut self, id_: i32) -> Result<(), DieselError> {
        let query = "
            BEGIN TRANSACTION;
            DELETE FROM bookmarks WHERE id = 2;
            UPDATE bookmarks SET id = id - 1 WHERE id > 2;
            COMMIT;
        ";
        self.conn.batch_execute(query)?;
        debug!(
            "({}:{}) Deleted and Compacted {:?}",
            function_name!(),
            line!(),
            id_
        );
        Ok(())
    }
    pub fn delete_bookmark2(&mut self, id_: i32) -> Result<usize, DieselError> {
        sql_query("BEGIN TRANSACTION;").execute(&mut self.conn)?;

        // Gotcha: 'returning *' not working within transaction
        let n = sql_query(
            "
            DELETE FROM bookmarks
            WHERE id = ?;
        ",
        )
        .bind::<Integer, _>(id_)
        .execute(&mut self.conn);
        debug!("({}:{}) Deleting {:?}", function_name!(), line!(), id_);

        // database compaction
        sql_query(
            "
            UPDATE bookmarks
            SET id = id - 1
            WHERE id > ?;
        ",
        )
        .bind::<Integer, _>(id_)
        .execute(&mut self.conn)?;
        debug!("({}:{}) {:?}", function_name!(), line!(), "Compacting");

        sql_query("COMMIT;").execute(&mut self.conn)?;
        debug!(
            "({}:{}) Deleted and Compacted, n: {:?}",
            function_name!(),
            line!(),
            n
        );
        n
    }
    pub fn clean_table(&mut self) -> Result<(), DieselError> {
        sql_query("DELETE FROM bookmarks WHERE id != 1;").execute(&mut self.conn)?;
        debug!("({}:{}) {:?}", function_name!(), line!(), "Cleaned table.");
        Ok(())
    }
    //noinspection RsTraitObligations
    pub fn update_bookmark(&mut self, bm: Bookmark) -> Result<Vec<Bookmark>, DieselError> {
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
    }

    //noinspection RsTraitObligations
    pub fn insert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>, DieselError> {
        diesel::insert_into(bookmarks)
            .values(bm)
            .get_results(&mut self.conn)
    }

    pub fn upsert_bookmark(&mut self, new_bm: NewBookmark) -> Result<Vec<Bookmark>, DieselError> {
        let bm = self.get_bookmark_by_url(&new_bm.URL);
        // if bm exists, update, else insert
        match bm {
            // update
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
            // insert
            Err(_) => self.insert_bookmark(new_bm),
        }
    }

    pub fn get_bookmark_by_id(&mut self, id_: i32) -> Result<Bookmark, DieselError> {
        // Ok(sql_query("SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks").load::<Bookmark2>(conn)?)
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash FROM bookmarks \
            where id = ?;",
        );
        
        bms.bind::<Integer, _>(id_).get_result(&mut self.conn)
    }
    pub fn get_bookmark_by_url(&mut self, url: &str) -> Result<Bookmark, DieselError> {
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash FROM bookmarks \
            where URL = ?;",
        );
        
        bms.bind::<Text, _>(url).get_result(&mut self.conn)
    }
    pub fn get_bookmarks(&mut self, query: &str) -> Result<Vec<Bookmark>, DieselError> {
        if query.is_empty() {
            // select all
            return bookmarks.load::<Bookmark>(&mut self.conn);
        }
        let ids = self.get_bookmarks_fts(query)?;

        // Query the bookmarks table for the full Bookmark objects
        bookmarks
            .filter(id.eq_any(ids))
            .load::<Bookmark>(&mut self.conn)
    }

    pub fn get_bookmarks_fts(&mut self, fts_query: &str) -> Result<Vec<i32>, DieselError> {
        let ids = sql_query(
            "SELECT id FROM bookmarks_fts \
        WHERE bookmarks_fts MATCH ? \
        ORDER BY rank",
        )
        .bind::<Text, _>(fts_query)
        .load::<IdResult>(&mut self.conn)?
        .into_iter()
        .map(|result| result.id)
        .collect();

        Ok(ids)
    }

    pub fn get_bookmarks_without_embedding(&mut self) -> Result<Vec<Bookmark>, DieselError> {
        let result = bookmarks
            .filter(embedding.is_null())
            .load::<Bookmark>(&mut self.conn)?;

        Ok(result)
    }

    pub fn bm_exists(&mut self, url: &str) -> Result<bool, DieselError> {
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash FROM bookmarks \
            where URL = ?;",
        );
        let bms = bms
            .bind::<Text, _>(url)
            .get_results::<Bookmark>(&mut self.conn)?;
        Ok(!bms.is_empty())
    }

    /// get frequency based ordered list of all tags
    pub fn get_all_tags(&mut self) -> Result<Vec<TagsFrequency>, DieselError> {
        let tags_query = sql_query(
            "
            -- name: get_all_tags
            with RECURSIVE split(tags, rest) AS (
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
            group by tags
            ORDER BY 2 desc;
        ",
        );
        
        tags_query.get_results(&mut self.conn)
    }

    /// get ordered vector of tags
    pub fn get_all_tags_as_vec(&mut self) -> Result<Vec<String>, anyhow::Error> {
        let all_tags = self.get_all_tags().context("Error getting all tags")?;
        let mut all_tags: Vec<String> = all_tags.into_iter().map(|t| t.tag).collect();
        dlog2!("{:?}", all_tags);
        all_tags.sort();
        Ok(all_tags)
    }
    /// get frequency based ordered list of related tags for a given tag
    pub fn get_related_tags(&mut self, tag: &str) -> Result<Vec<TagsFrequency>, DieselError> {
        let search_tag = format!("%,{},%", tag);
        let tags_query = sql_query(
            "
            -- name: get_related_tags
            with RECURSIVE split(tags, rest) AS (
                SELECT '', tags || ','
                FROM bookmarks
                WHERE tags LIKE :tag_query
                -- WHERE tags LIKE ?
                UNION ALL
                SELECT substr(rest, 0, instr(rest, ',')),
                       substr(rest, instr(rest, ',') + 1)
                FROM split
                WHERE rest <> '')
            SELECT tags as tag, count(tags) as n
            FROM split
            WHERE tags <> ''
            group by tags
            ORDER BY 2 desc;
        ",
        );
        
        tags_query
            .bind::<Text, _>(search_tag)
            .get_results(&mut self.conn)
    }

    pub fn get_randomized_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError> {
        let bms = sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY RANDOM()
            LIMIT ?;",
        );

        
        bms.bind::<Integer, _>(n).get_results(&mut self.conn)
    }

    pub fn get_oldest_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError> {
        let bms = sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY last_update_ts ASC
            LIMIT ?;",
        );

        
        bms.bind::<Integer, _>(n).get_results(&mut self.conn)
    }

    /// create a column "diesel_exists" in result which is the diesel "target"
    pub fn check_schema_migrations_exists(&mut self) -> Result<bool, DieselError> {
        let query = "
            SELECT 1 as diesel_exists FROM sqlite_master WHERE type='table' AND name='__diesel_schema_migrations';
        ";

        let result: Vec<ExistenceCheck> = sql_query(query).load(&mut self.conn)?;
        // Explicitly use the diesel_exists field to placate the compiler warning
        for item in &result {
            let _ = item.diesel_exists;
        }

        Ok(!result.is_empty())
    }

    pub fn check_embedding_column_exists(&mut self) -> Result<bool, DieselError> {
        let query = "
        SELECT COUNT(*) as column_exists
        FROM pragma_table_info('bookmarks')
        WHERE name='embedding';
    ";

        let result: Vec<ColumnCheck> = sql_query(query).load(&mut self.conn)?;

        // Check if any row was returned
        Ok(result.iter().any(|item| item.column_exists > 0))
    }
}

#[derive(QueryableByName, Debug)]
struct ExistenceCheck {
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
