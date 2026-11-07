-- History is keyed on the bookmark ID, so an ID must keep meaning one bookmark. A plain INTEGER
-- PRIMARY KEY gives a freed highest ID to the next row, and AUTOINCREMENT is the only way to stop
-- that. SQLite cannot add it to an existing table, hence the rebuild. Rowids stay as they are, so
-- vec_bookmarks still matches. Dropping the table drops its triggers.
CREATE TABLE bookmarks_new (
    id             INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    URL            VARCHAR NOT NULL UNIQUE,
    metadata       VARCHAR NOT NULL DEFAULT '',
    tags           VARCHAR NOT NULL DEFAULT '',
    "desc"         VARCHAR NOT NULL DEFAULT '',
    flags          INTEGER NOT NULL DEFAULT 0,
    last_update_ts DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    embedding      BLOB,
    content_hash   BLOB,
    created_ts     DATETIME DEFAULT NULL,
    embeddable     BOOLEAN NOT NULL DEFAULT 0,
    file_path      TEXT NULL,
    file_mtime     INTEGER NULL,
    file_hash      TEXT NULL,
    opener         TEXT NULL,
    accessed_at    DATETIME DEFAULT NULL
);

INSERT INTO bookmarks_new (id, URL, metadata, tags, "desc", flags, last_update_ts, embedding,
                           content_hash, created_ts, embeddable, file_path, file_mtime, file_hash,
                           opener, accessed_at)
SELECT id, URL, metadata, tags, "desc", flags, last_update_ts, embedding,
       content_hash, created_ts, embeddable, file_path, file_mtime, file_hash,
       opener, accessed_at
FROM bookmarks;

DROP TABLE bookmarks;
ALTER TABLE bookmarks_new RENAME TO bookmarks;

-- id was indexed, but the triggers never supply it. Entries from an old index, and from any 'rebuild'
-- (which purge runs), outlived their row, and a search for that number then failed with 'database
-- disk image is malformed'. A bare number no longer matches a bookmark's ID; use bkmr show for that.
-- The index only derives from bookmarks, so building it anew loses nothing.
DROP TABLE bookmarks_fts;
CREATE VIRTUAL TABLE bookmarks_fts USING fts5
(
    id UNINDEXED,
    URL,
    metadata,
    tags,
    "desc",
    flags UNINDEXED,
    last_update_ts UNINDEXED,
    content= 'bookmarks',
    content_rowid= 'id',
    tokenize= "porter unicode61"
);
INSERT INTO bookmarks_fts (bookmarks_fts) VALUES ('rebuild');

CREATE TRIGGER bookmarks_ad
    AFTER DELETE
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
END;

CREATE TRIGGER bookmarks_ai
    AFTER INSERT
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;

CREATE TRIGGER bookmarks_au
    AFTER UPDATE
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;

-- Old states of agent memories, for undo. Kept apart from bookmarks so that they stay out of
-- full-text and semantic search. No foreign key: an entry must outlive its deleted bookmark.
-- Creation time and access count are left out: history keeps what a bookmark said and how it opens.
CREATE TABLE bookmark_history (
    history_id  INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    bookmark_id INTEGER NOT NULL,
    change      TEXT NOT NULL,
    URL         TEXT NOT NULL,
    metadata    TEXT NOT NULL,
    tags        TEXT NOT NULL,
    desc        TEXT NOT NULL,
    changed_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    embeddable  BOOLEAN NOT NULL,
    file_path   TEXT NULL,
    file_mtime  INTEGER NULL,
    file_hash   TEXT NULL,
    opener      TEXT NULL
);
CREATE INDEX idx_bookmark_history_bookmark_id ON bookmark_history (bookmark_id);

-- Triggers rather than application code, so that undo covers older binaries and bulk rewrites, and
-- no config can switch it off for unattended agents. The one known gap: INSERT OR REPLACE fires no
-- delete trigger. Nothing in bkmr uses it. Only _mem_: deleting other types, _env_ above all, should
-- remove them. To add a type, replace these triggers in a new migration. Access counts, embeddings
-- and timestamps do not count as a change worth undoing.
--
-- The tag test accepts what Tag::parse_tags reads as _mem_, not only the canonical ',_mem_,'. An
-- update also counts if the new state is a memory: a restore can overwrite a row that had lost the
-- tag, and that text must not disappear.
CREATE TRIGGER bookmarks_history_au
    AFTER UPDATE
    ON bookmarks
    WHEN (instr(',' || lower(replace(old.tags, ' ', '')) || ',', ',_mem_,') > 0
            OR instr(',' || lower(replace(new.tags, ' ', '')) || ',', ',_mem_,') > 0)
        AND (old.URL IS NOT new.URL
            OR old.metadata IS NOT new.metadata
            OR old.tags IS NOT new.tags
            OR old.desc IS NOT new.desc)
BEGIN
    INSERT INTO bookmark_history (bookmark_id, change, URL, metadata, tags, desc, embeddable,
                                  file_path, file_mtime, file_hash, opener)
    VALUES (old.id, 'update', old.URL, old.metadata, old.tags, old.desc, old.embeddable,
            old.file_path, old.file_mtime, old.file_hash, old.opener);
END;

CREATE TRIGGER bookmarks_history_ad
    AFTER DELETE
    ON bookmarks
    WHEN instr(',' || lower(replace(old.tags, ' ', '')) || ',', ',_mem_,') > 0
BEGIN
    INSERT INTO bookmark_history (bookmark_id, change, URL, metadata, tags, desc, embeddable,
                                  file_path, file_mtime, file_hash, opener)
    VALUES (old.id, 'delete', old.URL, old.metadata, old.tags, old.desc, old.embeddable,
            old.file_path, old.file_mtime, old.file_hash, old.opener);
END;
