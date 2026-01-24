-- Add opener column to bookmarks table for custom URL open commands
ALTER TABLE bookmarks ADD COLUMN opener TEXT NULL;
