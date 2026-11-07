DROP TRIGGER bookmarks_au;
CREATE TRIGGER bookmarks_au
    AFTER UPDATE
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;

-- Before the column: SQLite refuses to drop a column that a trigger refers to
DROP TRIGGER bookmarks_verified_au;
ALTER TABLE bookmarks DROP COLUMN verified_at;
