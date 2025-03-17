// src/infrastructure/repositories/sqlite/models.rs
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::sql_types::{Binary, Integer, Text, Timestamp};
use diesel::deserialize::FromSql;
use diesel::sqlite::Sqlite;
use std::collections::HashSet;
use diesel::QueryableByName;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::tag::Tag;
use super::error::SqliteRepositoryError;

#[derive(QueryableByName, Debug)]
pub struct IdResult {
    #[diesel(sql_type = Integer)]
    pub id: i32,
}

#[derive(QueryableByName, Debug)]
pub struct TagFrequency {
    #[diesel(sql_type = Text)]
    pub tag: String,

    #[diesel(sql_type = Integer)]
    pub frequency: i32,
}

#[derive(Debug)]
pub struct BookmarkModel {
    pub url: String,
    pub title: String,
    pub description: String,
    pub tags: String,
    pub access_count: i32,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
}

#[derive(QueryableByName, Debug)]
pub struct BookmarkWithEmbedding {
    #[diesel(sql_type = Integer)]
    pub id: i32,

    #[diesel(sql_type = Text)]
    pub url: String,

    #[diesel(sql_type = Text)]
    pub title: String,

    #[diesel(sql_type = Text)]
    pub description: String,

    #[diesel(sql_type = Text)]
    pub tags: String,

    #[diesel(sql_type = Integer)]
    pub access_count: i32,

    #[diesel(sql_type = Timestamp)]
    pub created_at: NaiveDateTime,

    #[diesel(sql_type = Timestamp)]
    pub updated_at: NaiveDateTime,

    #[diesel(sql_type = Binary)]
    pub embedding: Option<Vec<u8>>,

    #[diesel(sql_type = Binary)]
    pub content_hash: Option<Vec<u8>>,
}

impl BookmarkWithEmbedding {
    pub fn into_domain(self) -> Result<Bookmark, SqliteRepositoryError> {
        // Parse tags
        let tags = Tag::parse_tags(&self.tags)
            .map_err(|e| SqliteRepositoryError::ConversionError(e.to_string()))?;

        // Create datetime from naive datetime
        let created_at = DateTime::<Utc>::from_naive_utc_and_offset(self.created_at, Utc);
        let updated_at = DateTime::<Utc>::from_naive_utc_and_offset(self.updated_at, Utc);

        // Create domain Bookmark
        let mut bookmark = Bookmark::from_storage(
            self.id,
            self.url,
            self.title,
            self.description,
            self.tags,
            self.access_count,
            created_at,
            updated_at,
        ).map_err(|e| SqliteRepositoryError::ConversionError(e.to_string()))?;

        Ok(bookmark)
    }
}
