#![allow(non_snake_case)]

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sql_types::Text;

use super::schema::bookmarks;

#[derive(QueryableByName, Debug, PartialOrd, PartialEq)]
pub struct Tags {
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
    pub fn split_tags(&self) -> Vec<String> {
        self.tags
            .split(",")
            .filter(|x| *x != "")
            .map(|s| s.to_string())
            .collect()
    }
}

#[derive(Insertable)]
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
    fn test_split_tags(bm: Bookmark) {
        println!("{:?}", bm);
        assert_eq!(bm.split_tags(), vec!("aaa", "xxx"));
    }
}
