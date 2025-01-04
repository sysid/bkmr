#![crate_type = "lib"]
#![crate_name = "bkmr"]

extern crate skim;

use std::collections::HashSet;


use crate::adapter::dal::Dal;

use anyhow::Result;

use crate::environment::CONFIG;
use crate::model::bookmark::Bookmark;
use crate::model::bookmark::BookmarkUpdater;
use crate::model::tag::Tags;
use itertools::Itertools;
use reqwest::blocking::Client;
use select::document::Document;
use select::predicate::{Attr, Name};
use tracing::{debug, error};

pub mod adapter {
    pub mod dal;
    pub mod embeddings;
    pub mod json;
}

pub mod model {
    pub mod bms;
    pub mod bookmark;
    pub mod tag;
}

pub mod service {
    pub mod embeddings;
    pub mod fzf;
    pub mod process;
}

pub mod cli;
pub mod context;
pub mod environment;
pub mod util;

/// creates list of normalized tags from "tag1,t2,t3" string
/// be aware of shell parsing rules, so no blanks or quotes
pub fn load_url_details(url: &str) -> Result<(String, String, String)> {
    let client = Client::new();
    let body = client.get(url).send()?.text()?;

    let document = Document::from(body.as_str());
    // let document = Document::from(body.to_string());

    let title = document
        .find(Name("title"))
        .next()
        .map(|n| n.text().trim().to_owned())
        .unwrap_or_default();

    let description = document
        .find(Attr("name", "description"))
        .next()
        .and_then(|n| n.attr("content"))
        .unwrap_or_default();

    let keywords = document
        .find(Attr("name", "keywords"))
        .next()
        .and_then(|node| node.attr("content"))
        .unwrap_or_default();

    debug!("Keywords {:?}", keywords);

    Ok((title, description.to_owned(), keywords.to_owned()))
}

pub fn update_bookmarks(
    ids: Vec<i32>,
    tags: Vec<String>,
    tags_not: Vec<String>,
    force: bool,
) -> Result<()> {
    // let mut bms = Bookmarks::new("".to_string());
    let mut dal = Dal::new(CONFIG.db_url.clone());
    for id in ids {
        update_bm(id, &tags, &tags_not, &mut dal, force).map_err(|e| {
            // Adjust the error handling here as needed
            // If 'e' needs to be used or logged, do it here. If necessary, clone 'e'.
            // Example: log::error!("Error updating bookmark: {}", e);
            // Assuming 'e' implements the 'Error' trait and can be converted/cloned
            error!("Error updating bookmark {}: {}", id, e);
            e
        })?;
    }
    Ok(())
}

pub fn update_bm(
    id: i32,
    tags: &Vec<String>,
    tags_not: &Vec<String>,
    dal: &mut Dal,
    force: bool,
) -> Result<Vec<Bookmark>> {
    let tags: HashSet<String> = tags.iter().cloned().collect();
    let tags_not: HashSet<String> = tags_not.iter().cloned().collect();
    debug!("id {}, tags {:?}, tags_not {:?}", id, tags, tags_not);

    let bm = dal.get_bookmark_by_id(id)?;

    let new_tags = if force {
        tags
    } else {
        let mut new_tags = Tags::normalize_tag_string(Some(bm.tags.clone()))
            .into_iter()
            .collect::<HashSet<String>>();
        new_tags.extend(tags);
        new_tags
            .difference(&tags_not)
            .map(|s| s.to_string())
            .collect()
    };

    let bm_tags: Vec<String> = new_tags.iter().sorted().cloned().collect();
    debug!("bm_tags {:?}", bm_tags);

    let mut bm_updated = Bookmark {
        tags: format!(",{},", bm_tags.join(",")),
        flags: bm.flags + 1,
        ..bm
    };
    bm_updated.update();
    dal.update_bookmark(bm_updated)
        .map_err(|e| anyhow::anyhow!("Error updating bookmark: {:?}", e))
}

