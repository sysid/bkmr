ALTER TABLE bookmarks DROP COLUMN accessed_at;
CREATE TRIGGER [UpdateLastTime]
    AFTER UPDATE
    ON bookmarks
    FOR EACH ROW
    WHEN NEW.last_update_ts <= OLD.last_update_ts
BEGIN
    update bookmarks set last_update_ts=CURRENT_TIMESTAMP where id = OLD.id;
END;
