-- Create sqlite-vec virtual table for embedding storage
-- Requires sqlite-vec extension to be registered via sqlite3_auto_extension
-- Embeddings are 768-dimensional float vectors (NomicEmbedTextV15 default)
-- rowid corresponds to bookmarks.id
CREATE VIRTUAL TABLE IF NOT EXISTS vec_bookmarks USING vec0(
    embedding float[768]
);
