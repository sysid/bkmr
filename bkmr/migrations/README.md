diesel migration generate add_embedding_and_content_hash_to_bookmarks

/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db print-schema --except-tables bookmarks_fts
/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db migration run
/Users/Q187392/.cargo/bin/diesel --database-url sqlite://../db/bkmr.db migration revert
