-- Your SQL goes here
ALTER TABLE bookmarks ADD COLUMN embedding BLOB;
ALTER TABLE bookmarks ADD COLUMN content_hash BLOB;
