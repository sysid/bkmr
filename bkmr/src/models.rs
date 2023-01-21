#![allow(non_snake_case)]

use stdext::function_name;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sql_types::Text;
use log::debug;
use crate::tag::Tags;

use super::schema::bookmarks;

#[derive(QueryableByName, Debug, PartialOrd, PartialEq)]
pub struct TagsFrequency {
    #[diesel(sql_type = Integer)]
    pub n: i32,
    #[diesel(sql_type = Text)]
    pub tag: String,
}

#[derive(Queryable, QueryableByName, Debug, PartialOrd, PartialEq, Clone)]
#[diesel(table_name = bookmarks)]
pub struct Bookmark {
    pub id: i32,
    pub URL: String,
    pub metadata: String,
    pub tags: String,
    pub desc: String,
    pub flags: i32,
    pub last_update_ts: NaiveDateTime,
    // pub last_update_ts: DateTime<Utc>,
}

impl Bookmark {
    pub fn get_tags(&self) -> Vec<String> {
        Tags::normalize_tag_string(Some(self.tags.clone()))
    }
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = format!(",{},", Tags::clean_tags(tags).join(","));
        debug!("({}:{}) {:?}", function_name!(), line!(), self);
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
}

#[cfg(test)]
mod test {
    use crate::models::Bookmark;
    use chrono::NaiveDate;
    use rstest::*;

    #[fixture]
    fn bm() -> Bookmark {
        Bookmark {
            id: 1,
            URL: String::from("www.sysid.de"),
            metadata: String::from(""),
            tags: String::from(",aaa,xxx,"),
            desc: String::from(""),
            flags: 0,
            last_update_ts: NaiveDate::from_ymd_opt(2016, 7, 8)
                .unwrap()
                .and_hms_opt(9, 10, 11)
                .unwrap(),
        }
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
        bm.set_tags(vec!("zzz".to_string()));
        assert_eq!(bm.get_tags(), vec!("zzz".to_string()));
    }
}

