#![allow(unused_imports, unused_variables)]
use diesel::prelude::*;
use std::env::args;
use twbm::schema::bookmarks::dsl::bookmarks;
use twbm::models;
use twbm::dal::establish_connection;

fn main() {
    let id = args()
        .nth(1)
        .expect("publish_bookmark requires a bookmark id")
        .parse::<i32>()
        .expect("Invalid ID");
    let connection = &mut establish_connection();

    // let _ = diesel::update(bookmarks.find(id))
    //     .set(published.eq(true))
    //     .execute(connection)
    //     .unwrap();
    //
    // let bookmark: models::Bookmark = bookmarks
    //     .find(id)
    //     .first(connection)
    //     .unwrap_or_else(|_| panic!("Unable to find bookmark {}", id));
    //
    // println!("Published bookmark {}", bookmark.title);
}
