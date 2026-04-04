// bkmr/src/infrastructure/repositories/sqlite/vector_repository.rs
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::vector_repository::VectorRepository;
use rusqlite::Connection;
use std::collections::HashSet;
use std::sync::Mutex;
use tracing::{debug, instrument};
use zerocopy::AsBytes;

/// sqlite-vec backed implementation of VectorRepository.
/// Uses a separate rusqlite connection (not Diesel) because sqlite-vec
/// requires raw SQL with MATCH syntax that Diesel cannot express.
pub struct SqliteVectorRepository {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteVectorRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteVectorRepository").finish()
    }
}

impl SqliteVectorRepository {
    /// Open a rusqlite connection to the given database URL.
    /// The sqlite-vec extension must already be registered via sqlite3_auto_extension.
    pub fn new(db_url: &str) -> DomainResult<Self> {
        let conn = Connection::open(db_url).map_err(|e| {
            DomainError::RepositoryError(crate::domain::error::RepositoryError::Connection(
                format!("Failed to open rusqlite connection for vector repo: {}", e),
            ))
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn get_conn(
        &self,
    ) -> DomainResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Vector repository connection lock poisoned: {}",
                e
            ))
        })
    }
}

impl VectorRepository for SqliteVectorRepository {
    #[instrument(skip(self))]
    fn init_vec_table(&self, dimensions: usize) -> DomainResult<()> {
        let conn = self.get_conn()?;
        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_bookmarks USING vec0(embedding float[{}])",
            dimensions
        ))
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to create vec_bookmarks table: {}",
                e
            ))
        })?;
        debug!("vec_bookmarks table initialized with {} dimensions", dimensions);
        Ok(())
    }

    #[instrument(skip(self, embedding))]
    fn upsert_embedding(&self, bookmark_id: i32, embedding: &[f32]) -> DomainResult<()> {
        let conn = self.get_conn()?;
        // Delete existing entry if any, then insert
        conn.execute(
            "DELETE FROM vec_bookmarks WHERE rowid = ?1",
            rusqlite::params![bookmark_id],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to delete old embedding for bookmark {}: {}",
                bookmark_id, e
            ))
        })?;

        conn.execute(
            "INSERT INTO vec_bookmarks(rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![bookmark_id, embedding.as_bytes()],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to insert embedding for bookmark {}: {}",
                bookmark_id, e
            ))
        })?;

        debug!("Upserted embedding for bookmark {}", bookmark_id);
        Ok(())
    }

    #[instrument(skip(self))]
    fn delete_embedding(&self, bookmark_id: i32) -> DomainResult<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "DELETE FROM vec_bookmarks WHERE rowid = ?1",
            rusqlite::params![bookmark_id],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to delete embedding for bookmark {}: {}",
                bookmark_id, e
            ))
        })?;
        Ok(())
    }

    #[instrument(skip(self, query_embedding))]
    fn search_nearest(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> DomainResult<Vec<(i32, f64)>> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT rowid, distance \
                 FROM vec_bookmarks \
                 WHERE embedding MATCH ?1 \
                 ORDER BY distance \
                 LIMIT ?2",
            )
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to prepare vector search query: {}",
                    e
                ))
            })?;

        let results = stmt
            .query_map(
                rusqlite::params![query_embedding.as_bytes(), limit as i64],
                |row| Ok((row.get::<_, i32>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!("Vector search query failed: {}", e))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect vector search results: {}",
                    e
                ))
            })?;

        debug!("Vector search returned {} results", results.len());
        Ok(results)
    }

    fn has_embeddings(&self) -> DomainResult<bool> {
        let conn = self.get_conn()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM vec_bookmarks", [], |row| row.get(0))
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to check vec_bookmarks count: {}",
                    e
                ))
            })?;
        Ok(count > 0)
    }

    fn get_dimensions(&self) -> DomainResult<Option<usize>> {
        let conn = self.get_conn()?;
        // Read one row's embedding to determine byte length -> dimension count
        let result: Result<Vec<u8>, _> = conn.query_row(
            "SELECT embedding FROM vec_bookmarks LIMIT 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(bytes) => {
                // Each f32 is 4 bytes
                Ok(Some(bytes.len() / 4))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DomainError::BookmarkOperationFailed(format!(
                "Failed to detect vec_bookmarks dimensions: {}",
                e
            ))),
        }
    }

    #[instrument(skip(self))]
    fn clear_all(&self) -> DomainResult<()> {
        let conn = self.get_conn()?;
        conn.execute("DELETE FROM vec_bookmarks", []).map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to clear vec_bookmarks: {}",
                e
            ))
        })?;
        debug!("Cleared all embeddings from vec_bookmarks");
        Ok(())
    }

    fn get_embedded_ids(&self) -> DomainResult<HashSet<i32>> {
        let conn = self.get_conn()?;
        let mut stmt = conn
            .prepare("SELECT rowid FROM vec_bookmarks")
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to query embedded IDs: {}",
                    e
                ))
            })?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i32>(0))
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect embedded IDs: {}",
                    e
                ))
            })?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect embedded IDs: {}",
                    e
                ))
            })?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_repo() -> SqliteVectorRepository {
        // sqlite-vec extension must be registered before opening connection
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_bookmarks USING vec0(embedding float[4])",
        )
        .unwrap();

        SqliteVectorRepository {
            conn: Mutex::new(conn),
        }
    }

    #[test]
    fn given_empty_table_when_has_embeddings_then_returns_false() {
        let repo = setup_test_repo();
        assert!(!repo.has_embeddings().unwrap());
    }

    #[test]
    fn given_empty_table_when_get_dimensions_then_returns_none() {
        let repo = setup_test_repo();
        assert_eq!(repo.get_dimensions().unwrap(), None);
    }

    #[test]
    fn given_embedding_when_upsert_then_retrievable() {
        let repo = setup_test_repo();
        let embedding = vec![0.1f32, 0.2, 0.3, 0.4];
        repo.upsert_embedding(1, &embedding).unwrap();

        assert!(repo.has_embeddings().unwrap());
        assert_eq!(repo.get_dimensions().unwrap(), Some(4));
    }

    #[test]
    fn given_embedding_when_delete_then_gone() {
        let repo = setup_test_repo();
        repo.upsert_embedding(1, &[0.1f32, 0.2, 0.3, 0.4]).unwrap();
        repo.delete_embedding(1).unwrap();
        assert!(!repo.has_embeddings().unwrap());
    }

    #[test]
    fn given_embeddings_when_search_nearest_then_returns_ordered_by_distance() {
        let repo = setup_test_repo();
        repo.upsert_embedding(1, &[0.1f32, 0.1, 0.1, 0.1]).unwrap();
        repo.upsert_embedding(2, &[0.9f32, 0.9, 0.9, 0.9]).unwrap();
        repo.upsert_embedding(3, &[0.5f32, 0.5, 0.5, 0.5]).unwrap();

        // Query close to embedding 2
        let results = repo
            .search_nearest(&[0.8f32, 0.8, 0.8, 0.8], 3)
            .unwrap();

        assert_eq!(results.len(), 3);
        // Nearest should be bookmark 2 (0.9,0.9,0.9,0.9)
        assert_eq!(results[0].0, 2);
        // Distances should be ascending
        assert!(results[0].1 <= results[1].1);
        assert!(results[1].1 <= results[2].1);
    }

    #[test]
    fn given_embeddings_when_clear_all_then_empty() {
        let repo = setup_test_repo();
        repo.upsert_embedding(1, &[0.1f32, 0.2, 0.3, 0.4]).unwrap();
        repo.upsert_embedding(2, &[0.5f32, 0.6, 0.7, 0.8]).unwrap();
        repo.clear_all().unwrap();
        assert!(!repo.has_embeddings().unwrap());
    }

    #[test]
    fn given_embeddings_when_get_embedded_ids_then_returns_all_ids() {
        let repo = setup_test_repo();
        repo.upsert_embedding(10, &[0.1f32, 0.2, 0.3, 0.4]).unwrap();
        repo.upsert_embedding(20, &[0.5f32, 0.6, 0.7, 0.8]).unwrap();
        let ids = repo.get_embedded_ids().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&10));
        assert!(ids.contains(&20));
    }

    #[test]
    fn given_existing_embedding_when_upsert_same_id_then_replaces() {
        let repo = setup_test_repo();
        repo.upsert_embedding(1, &[0.1f32, 0.2, 0.3, 0.4]).unwrap();
        repo.upsert_embedding(1, &[0.9f32, 0.8, 0.7, 0.6]).unwrap();

        // Should still have only 1 entry
        let ids = repo.get_embedded_ids().unwrap();
        assert_eq!(ids.len(), 1);
    }
}
