#![allow(non_snake_case)]

use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sql_types::Text;
use serde::Serialize;
use std::fmt;
use tracing::debug;
use crate::util::helper::calc_content_hash;
use crate::model::tag::Tags;

use crate::adapter::dal::schema::bookmarks;
use crate::context::Context;
// ORM mappings

#[derive(QueryableByName)]
pub struct IdResult {
    #[diesel(sql_type = Integer)]
    pub id: i32,
}

#[derive(QueryableByName, Debug, PartialOrd, PartialEq)]
pub struct TagsFrequency {
    #[diesel(sql_type = Integer)]
    pub n: i32,
    #[diesel(sql_type = Text)]
    pub tag: String,
}

pub trait BookmarkUpdater {
    fn update(&mut self);
}

#[derive(Queryable, QueryableByName, PartialOrd, PartialEq, Clone, Default, Serialize)]
#[diesel(table_name = bookmarks)]
pub struct Bookmark {
    pub id: i32,
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    #[serde(with = "serde_with::chrono::NaiveDateTime")]
    pub last_update_ts: NaiveDateTime,
    // pub last_update_ts: DateTime<Utc>,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
}

impl fmt::Display for Bookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "id: {}, URL: {}, metadata: {}, tags: {}, desc: {}, flags: {}, last_update_ts: {}, embedding: {}, content_hash: {}",
            self.id,
            self.URL,
            self.metadata,
            self.tags,
            self.desc,
            self.flags,
            self.last_update_ts,
            self.embedding.as_ref()
                .map_or(String::from("None"), |v| format!("{:X?}", &v.iter().take(3).collect::<Vec<&u8>>())), // Truncate and hex format
            self.content_hash.as_ref()
                .map_or(String::from("None"), |v| format!("{:X?}", &v.iter().take(3).collect::<Vec<&u8>>())) // Truncate and hex format
        )
    }
}

impl Bookmark {
    pub fn get_tags(&self) -> Vec<String> {
        Tags::normalize_tag_string(Some(self.tags.clone()))
    }
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = format!(",{},", Tags::clean_tags(tags).join(","));
        debug!("{:?}", self);
    }
    /// Returns a formatted string containing the bookmark's tags, metadata, description, and tags again.
    /// tags are tried to emphasize by using twice
    pub fn get_content(&self) -> String {
        let tags = self
            .get_tags()
            .iter()
            .filter(|t| !t.starts_with('_') && !t.ends_with('_'))
            .map(|t| t.to_string())
            .collect::<Vec<_>>();
        let tags_str = format!(",{},", tags.join(","));
        format!("{}{} -- {}{}", tags_str, self.metadata, self.desc, tags_str)
    }
    pub fn has_content_changed(&self) -> bool {
        self.content_hash != Some(calc_content_hash(self.get_content().as_str()))
    }

    // /// Update the embedding and content_hash fields
    // pub fn update(&mut self) {
    //     if !self.has_content_changed() && self.embedding.is_some() {
    //         debug!("Embedding exists and is up-to-date");
    //         return;
    //     }
    //     let embedding = CTX
    //         .get()
    //         .expect("Error: CTX is not initialized")
    //         .get_embedding(self.get_content().as_str());
    //     self.embedding = embedding;
    //     self.content_hash = Some(calc_content_hash(self.get_content().as_str()));
    // }
    pub fn convert_to_new_bookmark(&self) -> NewBookmark {
        NewBookmark {
            URL: self.URL.clone(),
            metadata: self.metadata.clone(),
            tags: self.tags.clone(),
            desc: self.desc.clone(),
            flags: self.flags,
            embedding: self.embedding.clone(),
            content_hash: self.content_hash.clone(),
        }
    }
}

impl BookmarkUpdater for Bookmark {
    fn update(&mut self) {
        if !self.has_content_changed() && self.embedding.is_some() {
            // If content hasn't changed and an embedding exists, log and return early.
            debug!("Embedding exists and is up-to-date");
            return;
        }

        // Assuming `CTX` is a globally accessible context that can produce embeddings.
        // And `calc_content_hash` is a function that calculates the hash of the bookmark content.
        let embedding = Context::read_global()
            .get_embedding(self.get_content().as_str());

        self.embedding = embedding;
        self.content_hash = Some(calc_content_hash(self.get_content().as_str()));
    }
}

impl fmt::Debug for Bookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bookmark")
            .field("id", &self.id)
            .field("URL", &self.URL)
            .field("metadata", &self.metadata)
            .field("tags", &self.tags)
            .field("desc", &self.desc)
            .field("flags", &self.flags)
            .field("last_update_ts", &self.last_update_ts)
            .field(
                "embedding",
                &self.embedding.as_ref().map(|v| LastEntries(v)),
            )
            .field(
                "content_hash",
                &self.content_hash.as_ref().map(|v| LastEntries(v)),
            )
            .finish()
    }
}

struct LastEntries<'a, T>(&'a [T]);

impl<T: fmt::Debug> fmt::Debug for LastEntries<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().rev().take(10).rev()) // Reverse, take 10, then reverse again to maintain order
            .finish()
    }
}

#[derive(Insertable, Clone, Debug, PartialOrd, PartialEq)]
#[diesel(table_name = bookmarks)]
pub struct NewBookmark {
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    pub embedding: Option<Vec<u8>>,
    pub content_hash: Option<Vec<u8>>,
}

#[derive(Default, Debug, PartialOrd, PartialEq)]
pub struct BookmarkBuilder {
    id: i32,
    URL: String,
    metadata: String,
    tags: String,
    desc: String,
    flags: i32,
    last_update_ts: NaiveDateTime,
    embedding: Option<Vec<u8>>,
    content_hash: Option<Vec<u8>>,
}

impl BookmarkBuilder {
    pub fn new() -> Self {
        BookmarkBuilder {
            ..Default::default()
        }
    }
    pub fn id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }
    pub fn URL(mut self, URL: String) -> Self {
        self.URL = URL;
        self
    }
    pub fn metadata(mut self, metadata: String) -> Self {
        self.metadata = metadata;
        self
    }
    pub fn tags(mut self, tags: String) -> Self {
        self.tags = tags;
        self
    }
    pub fn desc(mut self, desc: String) -> Self {
        self.desc = desc;
        self
    }
    pub fn flags(mut self, flags: i32) -> Self {
        self.flags = flags;
        self
    }
    pub fn embedding(mut self, embedding: Option<Vec<u8>>) -> Self {
        self.embedding = embedding;
        self
    }

    pub fn build(self) -> Bookmark {
        let mut bm = Bookmark {
            id: self.id,
            URL: self.URL,
            metadata: self.metadata,
            tags: self.tags,
            desc: self.desc,
            flags: self.flags,
            last_update_ts: Utc::now().naive_utc(),
            embedding: self.embedding,
            content_hash: None,
        };
        bm.content_hash = Some(calc_content_hash(bm.get_content().as_str()));
        bm
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, NaiveDate};
    use rstest::*;

    use crate::util::helper::calc_content_hash;
    use crate::model::bookmark::Bookmark;

    #[fixture]
    fn bm() -> Bookmark {
        let mut bm = super::BookmarkBuilder::new()
            .id(1)
            .URL("www.sysid.de".to_string())
            .metadata("metadata".to_string())
            .tags(",aaa,xxx,".to_string())
            .desc("desc".to_string())
            .flags(0)
            .build();
        bm.last_update_ts = NaiveDate::from_ymd_opt(2016, 7, 8)
            .unwrap()
            .and_hms_opt(9, 10, 11)
            .unwrap();
        // bm.content_hash = Some(bm.calc_content_hash());
        bm
    }

    #[rstest]
    fn test_get_content(mut bm: Bookmark) {
        bm.tags = ",aaa,_imported_,xxx,".to_string();
        let expected_content = ",aaa,xxx,metadata -- desc,aaa,xxx,";
        let content = bm.get_content();
        assert_eq!(content, expected_content);
    }

    #[rstest]
    fn test_get_content_hash(bm: Bookmark) {
        let expected_content = ",aaa,xxx,metadata -- desc,aaa,xxx,";
        let expected_hash = md5::compute(expected_content).0.to_vec();
        assert_eq!(calc_content_hash(bm.get_content().as_str()), expected_hash);
    }

    #[rstest]
    fn test_has_content_changed(mut bm: Bookmark) {
        // Case 1: Content hasn't changed
        assert!(!bm.has_content_changed());

        // Case 2: Content has changed
        bm.metadata = "new metadata".to_string();
        assert!(bm.has_content_changed());
    }

    #[rstest]
    fn test_bm(bm: Bookmark) {
        println!("{:?}", bm);
    }

    #[rstest]
    fn test_get_tags(bm: Bookmark) {
        println!("{:?}", bm);
        assert_eq!(bm.get_tags(), vec!("aaa", "xxx"));
    }

    #[rstest]
    fn test_set_tags(mut bm: Bookmark) {
        println!("{:?}", bm);
        bm.set_tags(vec!["zzz".to_string()]);
        assert_eq!(bm.get_tags(), vec!("zzz".to_string()));
    }

    #[rstest]
    fn test_bm_builder() {
        let bm = super::BookmarkBuilder::new().build();
        println!("{:?}", bm);
        assert_eq!(bm.id, 0);
    }

    #[rstest]
    fn test_debug_output_empty_fields() {
        let bookmark = Bookmark {
            id: 1,
            URL: "http://example.com".to_string(),
            metadata: "metadata".to_string(),
            tags: "tag1, tag2".to_string(),
            desc: "description".to_string(),
            flags: 0,
            last_update_ts: DateTime::from_timestamp(60, 0).unwrap().naive_utc(),
            embedding: None,
            content_hash: None,
        };

        let debug_str = format!("{:?}", bookmark);
        assert!(debug_str.contains("embedding: None"));
        assert!(debug_str.contains("content_hash: None"));
    }

    #[rstest]
    fn test_debug_output_with_few_elements() {
        let bookmark = Bookmark {
            embedding: Some(vec![1, 2, 3]),
            content_hash: Some(vec![4, 5, 6]),
            ..Default::default() // Fill other fields with default values
        };

        let debug_str = format!("{:?}", bookmark);
        assert!(debug_str.contains("embedding: Some([1, 2, 3])"));
        assert!(debug_str.contains("content_hash: Some([4, 5, 6])"));
    }

    #[rstest]
    fn test_debug_output_with_many_elements() {
        let bookmark = Bookmark {
            embedding: Some((0..15).collect()),
            content_hash: Some((16..31).collect()),
            ..Default::default()
        };

        let debug_str = format!("{:?}", bookmark);
        assert!(debug_str.contains("embedding: Some([5, 6, 7, 8, 9, 10, 11, 12, 13, 14])"));
        assert!(debug_str.contains("content_hash: Some([21, 22, 23, 24, 25, 26, 27, 28, 29, 30])"));
    }
}
