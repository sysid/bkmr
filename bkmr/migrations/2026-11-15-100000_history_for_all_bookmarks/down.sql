-- Back to history for _mem_ only, as in 2026-11-07-100000_add_bookmark_history. Entries already
-- kept for other types stay: they are old states, and deleting them would lose undo.
DROP TRIGGER bookmarks_history_au;
DROP TRIGGER bookmarks_history_ad;

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
