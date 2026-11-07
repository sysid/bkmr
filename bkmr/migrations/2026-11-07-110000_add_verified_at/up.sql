-- When someone last checked a bookmark's current text against reality. Its own column because
-- updated_at moves with bulk rewrites and accessed_at with show, open and surprise, so neither can
-- mean this. Only `bkmr verify` sets it. NULL means not checked: use created_ts as the anchor, which
-- for a restored deleted memory is the time of the restore.
ALTER TABLE bookmarks ADD COLUMN verified_at DATETIME DEFAULT NULL;

-- A stamp says the text was checked, so it ends when the content or title changes. A trigger rather
-- than application code, so that restore, imports and writes outside bkmr are all covered.
CREATE TRIGGER bookmarks_verified_au
    AFTER UPDATE OF URL, metadata
    ON bookmarks
    WHEN old.URL IS NOT new.URL OR old.metadata IS NOT new.metadata
BEGIN
    UPDATE bookmarks SET verified_at = NULL WHERE id = new.id;
END;

-- Only the indexed columns: a bare AFTER UPDATE rewrote the row's full text in the index for every
-- stamp and every recorded access.
DROP TRIGGER bookmarks_au;
CREATE TRIGGER bookmarks_au
    AFTER UPDATE OF URL, metadata, tags, "desc"
    ON bookmarks
BEGIN
    INSERT INTO bookmarks_fts (bookmarks_fts, rowid, URL, metadata, tags, "desc")
    VALUES ('delete', old.id, old.URL, old.metadata, old.tags, old.desc);
    INSERT INTO bookmarks_fts (rowid, URL, metadata, tags, "desc")
    VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
END;
