DROP TRIGGER bookmarks_history_ad;
DROP TRIGGER bookmarks_history_au;
DROP TABLE bookmark_history;
-- bookmarks keeps AUTOINCREMENT: older binaries work with it, and rebuilding the table again to lose
-- a stricter ID rule would risk data for no gain. Running up.sql again rebuilds it cleanly.
