-- Add file_path and file_mtime columns to bookmarks table
ALTER TABLE bookmarks ADD COLUMN file_path TEXT NULL;
ALTER TABLE bookmarks ADD COLUMN file_mtime INTEGER NULL;
ALTER TABLE bookmarks ADD COLUMN file_hash Text NULL;
