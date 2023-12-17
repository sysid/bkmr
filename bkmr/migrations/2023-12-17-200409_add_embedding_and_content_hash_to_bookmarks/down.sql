-- This file should undo anything in `up.sql`
ALTER TABLE bookmarks DROP COLUMN embedding;
ALTER TABLE bookmarks DROP COLUMN content_hash;
