#![allow(unused_imports, unused_variables)]
use diesel::prelude::*;
use std::env::args;
use bkmr::dal::establish_connection;
use bkmr::schema::bookmarks::dsl::bookmarks;
use bkmr::schema::bookmarks::URL;

fn main() {
    let target = args().nth(1).expect("Expected a target to match against");
    let pattern = format!("%{}%", target);

    let connection = &mut establish_connection();
    let num_deleted = diesel::delete(bookmarks.filter(URL.like(pattern)))
        .execute(connection)
        .expect("Error deleting bookmarks");

    println!("Deleted {} bookmarks", num_deleted);
}
