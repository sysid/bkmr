# Diesel Migrations

diesel migration generate add_embedding_and_content_hash_to_bookmarks

/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db print-schema
Out: Diesel only supports tables with primary keys. Table bookmarks_fts has no primary key

/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db print-schema --except-tables bookmarks_fts
// @generated automatically by Diesel CLI.

diesel::table! {
    bookmarks (id) {
        id -> Integer,
        URL -> Text,
        metadata -> Text,
        tags -> Text,
        desc -> Text,
        flags -> Integer,
        last_update_ts -> Timestamp,
        embedding -> Nullable<Binary>,
        content_hash -> Nullable<Binary>,
    }
}


/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db migration run
/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db migration revert

