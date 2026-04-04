-- vec_bookmarks virtual table is created at runtime by SqliteVectorRepository::init_vec_table()
-- with the correct dimensions for the configured embedding model.
-- The table is NOT created here because:
-- 1. The dimension depends on the configured model (768 for Nomic, 384 for MiniLM, etc.)
-- 2. sqlite-vec virtual tables don't participate in Diesel's schema management
-- 3. init_vec_table handles CREATE, dimension mismatch detection, and DROP+recreate

-- Clear legacy embedding blobs from bookmarks table.
-- Embeddings are now stored in the vec_bookmarks virtual table (sqlite-vec).
UPDATE bookmarks SET embedding = NULL WHERE embedding IS NOT NULL;
