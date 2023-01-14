use std::collections::HashSet;

use log::debug;
use stdext::function_name;

use crate::dal::Dal;
use crate::environment::CONFIG;
use crate::models::Bookmark;
use crate::{match_all_tags, match_any_tags, match_exact_tags, normalize_tag_string};

#[derive(Debug)]
pub struct Bookmarks {
    dal: Dal,
    #[allow(dead_code)]
    fts_query: String,
    pub bms: Vec<Bookmark>,
}

// #[allow(dead_code)]
impl Bookmarks {
    pub fn new(fts_query: String) -> Self {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        Bookmarks {
            fts_query: fts_query.clone(),
            bms: dal
                .get_bookmarks(fts_query.as_str())
                .expect("Error getting bookmarks"),
            dal,
        }
    }
    pub fn check_tags(&mut self, tags: Vec<String>) -> Vec<String> {
        let all_tags: HashSet<String> = HashSet::from_iter(self.dal.get_all_tags_as_vec());
        let tags = HashSet::from_iter(tags.into_iter().filter(|s| *s != ""));
        debug!("({}:{}) {:?}", function_name!(), line!(), all_tags);
        tags.difference(&all_tags).map(|s| s.to_string()).collect()
    }

    pub fn match_all(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!(
            "({}:{}) {:?} {:?} {:?}",
            function_name!(),
            line!(),
            tags,
            bms,
            not
        );
        match not {
            false => bms
                .into_iter()
                .filter(|bm| match_all_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !match_all_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
        }
    }
    pub fn match_any(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!(
            "({}:{}) {:?} {:?} {:?}",
            function_name!(),
            line!(),
            tags,
            bms,
            not
        );
        match not {
            false => bms
                .into_iter()
                .filter(|bm| match_any_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !match_any_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
        }
    }
    pub fn match_exact(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!(
            "({}:{}) {:?} {:?} {:?}",
            function_name!(),
            line!(),
            tags,
            bms,
            not
        );
        match not {
            false => bms
                .into_iter()
                .filter(|bm| match_exact_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !match_exact_tags(&tags, &bm.split_tags().into_iter().collect()))
                .collect(),
        }
    }
    pub fn filter(
        &mut self,
        tags_all: Option<String>,
        tags_any: Option<String>,
        tags_all_not: Option<String>,
        tags_any_not: Option<String>,
        tags_exact: Option<String>,
    ) {
        let tags_all_ = normalize_tag_string(tags_all);
        let tags_any_ = normalize_tag_string(tags_any);
        let tags_all_not_ = normalize_tag_string(tags_all_not);
        let tags_any_not_ = normalize_tag_string(tags_any_not);
        let tags_exact_ = normalize_tag_string(tags_exact);

        if !tags_exact_.is_empty() {
            self.bms = Bookmarks::match_exact(tags_exact_, self.bms.clone(), false);
        } else {
            if !tags_all_.is_empty() {
                self.bms = Bookmarks::match_all(tags_all_, self.bms.clone(), false);
            }
            if !tags_any_.is_empty() {
                self.bms = Bookmarks::match_any(tags_any_, self.bms.clone(), false);
            }
            if !tags_any_not_.is_empty() {
                self.bms = Bookmarks::match_any(tags_any_not_, self.bms.clone(), true);
            }
            if !tags_all_not_.is_empty() {
                self.bms = Bookmarks::match_all(tags_all_not_, self.bms.clone(), true);
            }
        }
        debug!("({}:{}) {:?}", function_name!(), line!(), self.bms);
    }
}

#[cfg(test)]
mod test {
    use log::debug;
    use rstest::*;

    use super::*;

    #[ctor::ctor]
    fn init() {
        let _ = env_logger::builder()
            // Include all events in tests
            .filter_level(log::LevelFilter::max())
            // Ensure events are captured by `cargo test`
            .is_test(true)
            // Ignore errors initializing the logger if tests race to configure it
            .try_init();
    }

    #[rstest]
    fn test_init_bms() {
        let bms = Bookmarks::new("".to_string());
        assert_eq!(bms.bms.len(), 11);
    }

    #[rstest]
    #[case(vec![String::from("aaa"), String::from("bbb")], 0)]
    #[case(vec![String::from("xyz")], 1)]
    #[case(vec![String::from("")], 0)]
    #[case(vec![], 0)]
    fn test_check_tags(#[case] tags: Vec<String>, #[case] expected: usize) {
        let mut bms = Bookmarks::new("".to_string());
        let unknown_tags = bms.check_tags(tags);
        debug!("{:?}", unknown_tags);
        assert_eq!(unknown_tags.len(), expected);
    }

    #[rstest]
    fn test_match_all() {
        let mut bms = Bookmarks::new("".to_string());
        bms.filter(Some(",xxx,yyy,".to_string()), None, None, None, None);
        assert_eq!(bms.bms.len(), 1);
        assert_eq!(bms.bms[0].id, 2);
    }
    #[rstest]
    fn test_match_all_not() {
        let mut bms = Bookmarks::new("".to_string());
        bms.filter(None, None, Some(",xxx,yyy,".to_string()), None, None);
        assert_eq!(bms.bms.len(), 10);
        assert_ne!(bms.bms[0].id, 2);
    }
    #[rstest]
    fn test_match_any() {
        let mut bms = Bookmarks::new("".to_string());
        bms.filter(None, Some(",xxx,ccc,".to_string()), None, None, None);
        assert_eq!(bms.bms.len(), 4);
    }
    #[rstest]
    fn test_match_any_not() {
        let mut bms = Bookmarks::new("".to_string());
        bms.filter(None, None, None, Some(",xxx,ccc,".to_string()), None);
        assert_eq!(bms.bms.len(), 7);
    }
    #[rstest]
    fn test_match_exact() {
        let mut bms = Bookmarks::new("".to_string());
        bms.filter(None, None, None, None, Some(",aaa,bbb,".to_string()));
        assert_eq!(bms.bms.len(), 2);
    }
}
