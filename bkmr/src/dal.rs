use std::fmt;
use std::fmt::Debug;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{Integer, Text};
use diesel::{sql_query, Connection, RunQueryDsl, SqliteConnection};
use log::debug;
use stdext::function_name;

use crate::models::{Bookmark, NewBookmark, TagsFrequency};
use crate::schema::bookmarks::dsl::bookmarks;
use crate::schema::bookmarks::{desc, flags, id, metadata, tags, URL};

// use crate::schema::bookmarks;

// #[derive(Debug)]
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
    pub fn update_bookmark(&mut self, bm: Bookmark) -> Result<Vec<Bookmark>, DieselError> {
        diesel::update(bookmarks.find(bm.id))
            .set((
                URL.eq(bm.URL),
                metadata.eq(bm.metadata),
                tags.eq(bm.tags),
                desc.eq(bm.desc),
                flags.eq(bm.flags),
            ))
            .get_results(&mut self.conn)
    }

    pub fn insert_bookmark(&mut self, bm: NewBookmark) -> Result<Vec<Bookmark>, DieselError> {
        diesel::insert_into(bookmarks)
            .values(bm)
            .get_results(&mut self.conn)
    }

    pub fn get_bookmark_by_id(&mut self, id_: i32) -> Result<Bookmark, DieselError> {
        // Ok(sql_query("SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks").load::<Bookmark2>(conn)?)
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks \
            where id = ?;",
        );
        let bm = bms.bind::<Integer, _>(id_).get_result(&mut self.conn);
        bm
    }
    pub fn get_bookmarks(&mut self, query: &str) -> Result<Vec<Bookmark>, DieselError> {
        if query.is_empty() {
            // select all
            return bookmarks.load::<Bookmark>(&mut self.conn);
        }
        self.get_bookmarks_fts(query)
    }

    pub fn get_bookmarks_fts(&mut self, fts_query: &str) -> Result<Vec<Bookmark>, DieselError> {
        // Ok(sql_query("SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks").load::<Bookmark2>(conn)?)
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks_fts \
            where bookmarks_fts match ? \
            order by rank",
        );
        let bms = bms.bind::<Text, _>(fts_query).get_results(&mut self.conn);
        bms
    }

    pub fn bm_exists(&mut self, url: &str) -> Result<bool, DieselError> {
        let bms = sql_query(
            "SELECT id, URL, metadata, tags, desc, flags, last_update_ts FROM bookmarks \
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
        let tags_result = tags_query.get_results(&mut self.conn);
        tags_result
    }

    /// get ordered vector of tags
    pub fn get_all_tags_as_vec(&mut self) -> Vec<String> {
        let all_tags = self.get_all_tags().unwrap(); //todo handle error
        let mut all_tags: Vec<String> = all_tags.into_iter().map(|t| t.tag).collect();
        debug!("({}:{}) {:?}", function_name!(), line!(), all_tags);
        all_tags.sort();
        all_tags
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
        let tags_result = tags_query
            .bind::<Text, _>(search_tag)
            .get_results(&mut self.conn);
        tags_result
    }

    pub fn get_randomized_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError> {
        let bms = sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY RANDOM()
            LIMIT ?;"
        );

        let bms = bms.bind::<Integer, _>(n).get_results(&mut self.conn);
        bms
    }

    pub fn get_oldest_bookmarks(&mut self, n: i32) -> Result<Vec<Bookmark>, DieselError> {
        let bms = sql_query(
            "SELECT *
            FROM bookmarks
            ORDER BY last_update_ts ASC
            LIMIT ?;"
        );

        let bms = bms.bind::<Integer, _>(n).get_results(&mut self.conn);
        bms
    }
}

impl Debug for Dal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.url)
    }
}
