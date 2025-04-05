-- This file should undo anything in `up.sql`
ALTER TABLE bookmarks DROP COLUMN created_ts;
ALTER TABLE bookmarks DROP COLUMN embeddable;

