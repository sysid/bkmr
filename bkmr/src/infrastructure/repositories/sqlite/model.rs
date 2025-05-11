use chrono::NaiveDateTime;
use diesel::sql_types::{Integer, Text};
use diesel::{AsChangeset, Identifiable, Insertable, Queryable, QueryableByName};
use std::{fmt, format, write};

#[derive(QueryableByName, Debug)]
pub struct IdResult {
    #[diesel(sql_type = Integer)]
    pub id: i32,
}

#[derive(Queryable, Identifiable, QueryableByName, Clone)]
#[diesel(table_name = crate::infrastructure::repositories::sqlite::schema::bookmarks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(non_snake_case)]
pub struct DbBookmark {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text, column_name = "URL")]
    pub url: String,
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
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Timestamp>)]
    pub created_ts: Option<NaiveDateTime>,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub embeddable: bool,
}

impl fmt::Display for DbBookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id: {}, URL: {}, metadata: {}, tags: {}, desc: {}, flags: {}, last_update_ts: {}, embedding: {}, content_hash: {}, created_ts: {}",
            self.id,
            self.url,
            self.metadata,
            self.tags,
            self.desc,
            self.flags,
            self.last_update_ts,
            self.embedding.as_ref()
                .map_or(String::from("None"), |v| format!("{:X?}", &v.iter().take(3).collect::<Vec<&u8>>())), // Truncate and hex format
            self.content_hash.as_ref()
                .map_or(String::from("None"), |v| format!("{:X?}", &v.iter().take(3).collect::<Vec<&u8>>())), // Truncate and hex format
            self.created_ts.map_or("None".to_string(), |ts| ts.to_string())
        )
    }
}

impl fmt::Debug for DbBookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the same format as Display
        write!(f, "{}", self)
    }
}

/// Changes for updating a bookmark
#[derive(AsChangeset, Debug)]
#[diesel(table_name = crate::infrastructure::repositories::sqlite::schema::bookmarks)]
pub struct DbBookmarkChanges {
    #[diesel(column_name = "URL")]
    pub url: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    #[diesel(treat_none_as_null = true)]
    pub embedding: Option<Vec<u8>>,
    #[diesel(treat_none_as_null = true)]
    pub content_hash: Option<Vec<u8>>,
    pub created_ts: Option<NaiveDateTime>,
    pub embeddable: bool,
}

impl fmt::Display for DbBookmarkChanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "URL: {}, metadata: {}, tags: {}, desc: {}, flags: {}, embedding: {}, content_hash: {}, created_ts: {}",
            self.url,
            self.metadata,
            self.tags,
            self.desc,
            self.flags,
            self.embedding
                .as_ref()
                .map_or(String::from("None"), |v| format!(
                    "{:X?}",
                    &v.iter().take(3).collect::<Vec<&u8>>()
                )), // Truncate and hex format
            self.content_hash
                .as_ref()
                .map_or(String::from("None"), |v| format!(
                    "{:X?}",
                    &v.iter().take(3).collect::<Vec<&u8>>()
                )), // Truncate and hex format
            self.created_ts.map_or("None".to_string(), |ts| ts.to_string())
        )
    }
}

/// New bookmark for insertion
#[derive(Insertable, Debug)]
#[diesel(table_name = crate::infrastructure::repositories::sqlite::schema::bookmarks)]
pub struct NewBookmark {
    #[diesel(column_name = "URL")]
    pub url: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
    pub created_ts: Option<NaiveDateTime>,
    pub embeddable: bool,
}

impl fmt::Display for NewBookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "URL: {}, metadata: {}, tags: {}, desc: {}, flags: {}, embedding: {}, content_hash: {}, created_ts: {}, embeddable: {}",
            self.url,
            self.metadata,
            self.tags,
            self.desc,
            self.flags,
            self.embedding
                .as_ref()
                .map_or(String::from("None"), |v| format!(
                    "{:X?}",
                    &v.iter().take(3).collect::<Vec<&u8>>()
                )), // Truncate and hex format
            self.content_hash
                .as_ref()
                .map_or(String::from("None"), |v| format!(
                    "{:X?}",
                    &v.iter().take(3).collect::<Vec<&u8>>()
                )), // Truncate and hex format
            self.created_ts.map_or("None".to_string(), |ts| ts.to_string()),
            self.embeddable
        )
    }
}

/// Tags frequency for aggregation queries
#[derive(QueryableByName, Debug)]
pub struct TagsFrequency {
    #[diesel(sql_type = Text)]
    pub tag: String,

    #[diesel(sql_type = Integer)]
    pub n: i32,
}
