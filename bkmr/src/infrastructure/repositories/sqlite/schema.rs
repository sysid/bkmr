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
        created_ts -> Nullable<Timestamp>,
        embeddable -> Bool,
        file_path -> Nullable<Text>,
        file_mtime -> Nullable<Integer>,
        file_hash -> Nullable<Text>,
        opener -> Nullable<Text>,
        accessed_at -> Nullable<Timestamp>,
        verified_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    bookmark_history (history_id) {
        history_id -> Integer,
        bookmark_id -> Integer,
        change -> Text,
        URL -> Text,
        metadata -> Text,
        tags -> Text,
        desc -> Text,
        changed_at -> Timestamp,
        embeddable -> Bool,
        file_path -> Nullable<Text>,
        file_mtime -> Nullable<Integer>,
        file_hash -> Nullable<Text>,
        opener -> Nullable<Text>,
    }
}

diesel::table! {
    bookmarks_fts (id) {
        id -> Integer,
        URL -> Text,
        metadata -> Text,
        tags -> Text,
        desc -> Text,
        flags -> Integer,
        last_update_ts -> Timestamp,
    }
}
