-- History for every bookmark, not only _mem_: a plain delete can always be undone, and
-- `delete --force` is the one way to remove a bookmark for good, secrets included. One rule for all
-- types, with no exception for any tag, rather than undo that depends on a tag.
--
-- An update counts when what the bookmark says or how it opens changes: URL, title (metadata), tags,
-- description, opener or source file path. Access counts, verify stamps, embeddings, timestamps and
-- a file's mtime and hash do not: a touched file with unchanged content is no change worth undoing.
-- The one known gap: INSERT OR REPLACE fires no delete trigger. Nothing in bkmr uses it.
-- ponytail: history grows without bound; add a prune by age when the table gets large.
DROP TRIGGER bookmarks_history_au;
DROP TRIGGER bookmarks_history_ad;

CREATE TRIGGER bookmarks_history_au
    AFTER UPDATE
    ON bookmarks
    WHEN old.URL IS NOT new.URL
        OR old.metadata IS NOT new.metadata
        OR old.tags IS NOT new.tags
        OR old.desc IS NOT new.desc
        OR old.opener IS NOT new.opener
        OR old.file_path IS NOT new.file_path
BEGIN
    INSERT INTO bookmark_history (bookmark_id, change, URL, metadata, tags, desc, embeddable,
                                  file_path, file_mtime, file_hash, opener)
    VALUES (old.id, 'update', old.URL, old.metadata, old.tags, old.desc, old.embeddable,
            old.file_path, old.file_mtime, old.file_hash, old.opener);
END;

CREATE TRIGGER bookmarks_history_ad
    AFTER DELETE
    ON bookmarks
BEGIN
    INSERT INTO bookmark_history (bookmark_id, change, URL, metadata, tags, desc, embeddable,
                                  file_path, file_mtime, file_hash, opener)
    VALUES (old.id, 'delete', old.URL, old.metadata, old.tags, old.desc, old.embeddable,
            old.file_path, old.file_mtime, old.file_hash, old.opener);
END;
