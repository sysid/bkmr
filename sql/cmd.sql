CREATE TEMPORARY TABLE tmp_bookmarks(
    id INTEGER NOT NULL PRIMARY KEY,
    URL VARCHAR NOT NULL UNIQUE,
    metadata VARCHAR NOT NULL DEFAULT '',
    tags VARCHAR NOT NULL DEFAULT '',
    desc VARCHAR NOT NULL DEFAULT '',
    flags INTEGER NOT NULL DEFAULT 0,
    last_update_ts DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    embedding BLOB,
    content_hash BLOB
);
.mode csv
.import /Users/Q187392/dev/s/private/vimwiki/buku/bm.csv tmp_bookmarks

BEGIN transaction;
INSERT INTO bookmarks(id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash)
SELECT id, URL, metadata, tags, desc, flags, last_update_ts, embedding, content_hash
FROM tmp_bookmarks;
Commit;

DROP TABLE tmp_bookmarks;
