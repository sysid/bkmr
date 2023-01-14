#![allow(unused_imports, unused_variables)]
use diesel::prelude::*;
use twbm::dal::establish_connection;
use twbm::models::Bookmark;
use twbm::schema::bookmarks::dsl::bookmarks;

#[allow(unused_imports)]
use log::{debug, error, log_enabled, info, Level};
use twbm::schema::bookmarks::flags;

fn main() {
    env_logger::init();
    let connection = &mut establish_connection();
    let results = bookmarks
        .filter(flags.eq(0))
        .limit(5)
        .load::<Bookmark>(connection)
        .expect("Error loading bookmarks");

    println!("Displaying {} bookmarks", results.len());
    error!("Hello, world!");
    for bm in results {
        println!("{}", bm.URL);
        println!("----------\n");
        println!("{}", bm.tags);
    }
}
