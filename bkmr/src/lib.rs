#![crate_type = "lib"]
#![crate_name = "bkmr"]
// #![allow(unused_variables, unused_imports)]

extern crate skim;

use itertools::Itertools;
use log::{debug, error, warn};
use reqwest::blocking::Client;
use select::document::Document;
use select::predicate::{Attr, Name};
use std::collections::HashSet;

use crate::dal::Dal;
use crate::environment::CONFIG;
use crate::models::Bookmark;
use crate::tag::Tags;
#[allow(unused_imports)]
use stdext::function_name;

pub mod bms;
pub mod dal;
pub mod environment;
pub mod fzf;
pub mod helper;
pub mod models;
pub mod process;
pub mod schema;
pub mod tag;

/// creates list of normalized tags from "tag1,t2,t3" string
/// be aware of shell parsing rules, so no blanks or quotes
pub fn load_url_details(url: &str) -> Result<(String, String, String), anyhow::Error> {
    let client = Client::new();
    let body = client.get(url).send()?.text()?;

    let document = Document::from(body.as_str());
    // let document = Document::from(body.to_string());

    let title = document
        .find(Name("title"))
        .next()
        .and_then(|n| Some(n.text().trim().to_owned()))
        .unwrap_or_default();

    debug!("({}:{}) Title {:?}", function_name!(), line!(), title);

    let description = document
        .find(Attr("name", "description"))
        .next()
        .and_then(|n| n.attr("content"))
        .unwrap_or_default();
    debug!(
        "({}:{}) Description {:?}",
        function_name!(),
        line!(),
        description
    );

    let keywords = document
        .find(Attr("name", "keywords"))
        .next()
        .and_then(|node| node.attr("content"))
        .unwrap_or_default();

    debug!("({}:{}) Keywords {:?}", function_name!(), line!(), keywords);

    Ok((title, description.to_owned(), keywords.to_owned()))
}

pub fn update_bookmarks(ids: Vec<i32>, tags: Vec<String>, tags_not: Vec<String>, force: bool) {
    // let mut bms = Bookmarks::new("".to_string());

    let mut dal = Dal::new(CONFIG.db_url.clone());
    for id in ids {
        update_bm(id, &tags, &tags_not, &mut dal, force)
    }
}

pub fn update_bm(id: i32, tags: &Vec<String>, tags_not: &Vec<String>, dal: &mut Dal, force: bool) {
    let tags: HashSet<String> = tags.iter().cloned().collect();
    let tags_not: HashSet<String> = tags_not.iter().cloned().collect();
    debug!(
        "({}:{}) tags {:?}, tags_not {:?}",
        function_name!(),
        line!(),
        tags,
        tags_not
    );

    let bm = dal.get_bookmark_by_id(id);
    if let Err(e) = bm {
        warn!(
            "({}:{}) Cannot load {:?}, continue.",
            function_name!(),
            line!(),
            e
        );
        return;
    }
    let bm = bm.unwrap();

    let new_tags = if force {
        tags.clone()
    } else {
        let mut new_tags = Tags::normalize_tag_string(Some(bm.tags.clone()))
            .into_iter()
            .map(|s| s.to_string())
            .collect::<HashSet<String>>();
        new_tags.extend(tags);
        new_tags
            .difference(&tags_not)
            .map(|s| s.to_string())
            .collect()
    };

    let bm_tags: Vec<String> = new_tags.iter().sorted().cloned().collect();
    debug!("({}:{}) {:?}", function_name!(), line!(), bm_tags);

    let bm = dal.update_bookmark(Bookmark {
        tags: format!(",{},", bm_tags.join(",")),
        ..bm
    });
    if let Err(e) = bm {
        error!(
            "({}:{}) Error update {:?}, continue.",
            function_name!(),
            line!(),
            e
        );
        return;
    }
}

// pub fn add_bm(bm: Bookmark) {
//
// }

#[cfg(test)]
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

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use rstest::*;
}
