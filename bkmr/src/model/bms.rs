use std::collections::HashSet;

use crate::adapter::dal::Dal;
use crate::environment::CONFIG;
use crate::model::bookmark::Bookmark;
use crate::model::tag::Tags;
use anyhow::Result;
use tracing::debug;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Bookmarks {
    dal: Dal,
    fts_query: String,
    pub bms: Vec<Bookmark>,
}

impl Bookmarks {
    /// Creates a new instance of Bookmarks
    /// if query is empty, all bookmarks are loaded
    pub fn new(fts_query: String) -> Self {
        let mut dal = Dal::new(CONFIG.db_url.clone());
        Bookmarks {
            fts_query: fts_query.clone(),
            bms: dal
                .get_bookmarks(&fts_query)
                .expect("Error getting bookmarks"),
            dal,
        }
    }
    pub fn check_tags(&mut self, tags: Vec<String>) -> Result<Vec<String>> {
        let all_tags: HashSet<String> = HashSet::from_iter(self.dal.get_all_tags_as_vec()?);
        let tags = HashSet::from_iter(tags.into_iter().filter(|s| !s.is_empty()));
        debug!("{:?}", tags);
        Ok(tags.difference(&all_tags).cloned().collect())
    }

    pub fn match_all(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!("{:?} {:?} {:?}", tags, bms, not);
        match not {
            false => bms
                .into_iter()
                // .filter(|bm| Tags::match_all_tags(&tags, &bm.get_tags().into_iter().collect()))
                .filter(|bm| Tags::match_all_tags(&tags, &bm.get_tags()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !Tags::match_all_tags(&tags, &bm.get_tags()))
                .collect(),
        }
    }
    pub fn match_any(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!("{:?} {:?} {:?}", tags, bms, not);
        match not {
            false => bms
                .into_iter()
                .filter(|bm| Tags::match_any_tags(&tags, &bm.get_tags()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !Tags::match_any_tags(&tags, &bm.get_tags()))
                .collect(),
        }
    }
    pub fn match_exact(tags: Vec<String>, bms: Vec<Bookmark>, not: bool) -> Vec<Bookmark> {
        debug!("{:?} {:?} {:?}", tags, bms, not);
        match not {
            false => bms
                .into_iter()
                .filter(|bm| Tags::match_exact_tags(&tags, &bm.get_tags()))
                .collect(),
            true => bms
                .into_iter()
                .filter(|bm| !Tags::match_exact_tags(&tags, &bm.get_tags()))
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
        let tags_all_ = Tags::normalize_tag_string(tags_all);
        let tags_any_ = Tags::normalize_tag_string(tags_any);
        let tags_all_not_ = Tags::normalize_tag_string(tags_all_not);
        let tags_any_not_ = Tags::normalize_tag_string(tags_any_not);
        let tags_exact_ = Tags::normalize_tag_string(tags_exact);

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
        debug!("{:?}", self.bms);
    }
}

