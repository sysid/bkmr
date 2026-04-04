// bkmr/src/infrastructure/repositories/sqlite/vector_repository.rs
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::vector_repository::VectorRepository;
use rusqlite::Connection;
use std::collections::HashSet;
use std::sync::Mutex;
use tracing::{debug, instrument};
use zerocopy::AsBytes;

/// sqlite-vec backed implementation of VectorRepository.
///
/// Uses a single rusqlite connection (not Diesel) because sqlite-vec
/// requires raw SQL with MATCH syntax that Diesel cannot express.
///
/// Connection handling: each public trait method locks the mutex once,
/// then passes `&Connection` to private helpers. No method calls another
/// trait method — this prevents mutex re-entrancy deadlocks.
pub struct SqliteVectorRepository {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteVectorRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteVectorRepository").finish()
    }
}

/// Lock the mutex, mapping poison errors to DomainError.
fn lock_conn(mutex: &Mutex<Connection>) -> DomainResult<std::sync::MutexGuard<'_, Connection>> {
    mutex.lock().map_err(|e| {
        DomainError::BookmarkOperationFailed(format!(
            "Vector repository connection lock poisoned: {}",
            e
        ))
    })
}

/// Read the dimension count from the first row in vec_bookmarks.
/// Returns None if the table is empty.
fn query_dimensions(conn: &Connection) -> DomainResult<Option<usize>> {
    match conn.query_row("SELECT embedding FROM vec_bookmarks LIMIT 1", [], |row| {
        let bytes: Vec<u8> = row.get(0)?;
        Ok(bytes.len() / 4) // each f32 is 4 bytes
    }) {
        Ok(dims) => Ok(Some(dims)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DomainError::BookmarkOperationFailed(format!(
            "Failed to detect vec_bookmarks dimensions: {}",
            e
        ))),
    }
}

impl SqliteVectorRepository {
    /// Open a rusqlite connection to the given database URL.
    /// The sqlite-vec extension must already be registered via register_sqlite_vec().
    pub fn new(db_url: &str) -> DomainResult<Self> {
        let conn = Connection::open(db_url).map_err(|e| {
            DomainError::RepositoryError(crate::domain::error::RepositoryError::Connection(
                format!("Failed to open rusqlite connection for vector repo: {}", e),
            ))
        })?;
        // WAL mode allows concurrent reads/writes from the Diesel pool and
        // this rusqlite connection without blocking.
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
            .map_err(|e| {
                DomainError::RepositoryError(crate::domain::error::RepositoryError::Connection(
                    format!("Failed to configure vector repo connection: {}", e),
                ))
            })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl VectorRepository for SqliteVectorRepository {
    #[instrument(skip(self))]
    fn init_vec_table(&self, dimensions: usize) -> DomainResult<()> {
        let conn = lock_conn(&self.conn)?;

        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='vec_bookmarks'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if table_exists {
            match query_dimensions(&conn)? {
                Some(dims) if dims != dimensions => {
                    eprintln!(
                        "Embedding model dimensions changed ({} -> {}), recreating vector table...",
                        dims, dimensions
                    );
                    conn.execute_batch("DROP TABLE IF EXISTS vec_bookmarks")
                        .map_err(|e| {
                            DomainError::BookmarkOperationFailed(format!(
                                "Failed to drop vec_bookmarks for dimension change: {}", e
                            ))
                        })?;
                }
                Some(dims) => {
                    debug!("vec_bookmarks exists with correct dimensions ({})", dims);
                    return Ok(());
                }
                None => {
                    debug!("vec_bookmarks exists (empty), dimensions assumed correct");
                    return Ok(());
                }
            }
        }

        conn.execute_batch(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS vec_bookmarks USING vec0(embedding float[{}])",
            dimensions
        ))
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to create vec_bookmarks table: {}", e
            ))
        })?;
        debug!("vec_bookmarks table created with {} dimensions", dimensions);
        Ok(())
    }

    #[instrument(skip(self, embedding))]
    fn upsert_embedding(&self, bookmark_id: i32, embedding: &[f32]) -> DomainResult<()> {
        let conn = lock_conn(&self.conn)?;

        conn.execute(
            "DELETE FROM vec_bookmarks WHERE rowid = ?1",
            rusqlite::params![bookmark_id],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to delete old embedding for bookmark {}: {}", bookmark_id, e
            ))
        })?;

        conn.execute(
            "INSERT INTO vec_bookmarks(rowid, embedding) VALUES (?1, ?2)",
            rusqlite::params![bookmark_id, embedding.as_bytes()],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to insert embedding for bookmark {}: {}", bookmark_id, e
            ))
        })?;

        debug!("Upserted embedding for bookmark {}", bookmark_id);
        Ok(())
    }

    #[instrument(skip(self))]
    fn delete_embedding(&self, bookmark_id: i32) -> DomainResult<()> {
        let conn = lock_conn(&self.conn)?;
        conn.execute(
            "DELETE FROM vec_bookmarks WHERE rowid = ?1",
            rusqlite::params![bookmark_id],
        )
        .map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to delete embedding for bookmark {}: {}", bookmark_id, e
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
        let conn = lock_conn(&self.conn)?;
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
                    "Failed to prepare vector search query: {}", e
                ))
            })?;

        let results = stmt
            .query_map(
                rusqlite::params![query_embedding.as_bytes(), limit as i64],
                |row| Ok((row.get::<_, i32>(0)?, row.get::<_, f64>(1)?)),
            )
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Vector search query failed: {}", e
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect vector search results: {}", e
                ))
            })?;

        debug!("Vector search returned {} results", results.len());
        Ok(results)
    }

    fn has_embeddings(&self) -> DomainResult<bool> {
        let conn = lock_conn(&self.conn)?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM vec_bookmarks", [], |row| row.get(0))
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to check vec_bookmarks count: {}", e
                ))
            })?;
        Ok(count > 0)
    }

    fn get_dimensions(&self) -> DomainResult<Option<usize>> {
        let conn = lock_conn(&self.conn)?;
        query_dimensions(&conn)
    }

    #[instrument(skip(self))]
    fn clear_all(&self) -> DomainResult<()> {
        let conn = lock_conn(&self.conn)?;
        conn.execute("DELETE FROM vec_bookmarks", []).map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to clear vec_bookmarks: {}", e
            ))
        })?;
        debug!("Cleared all embeddings from vec_bookmarks");
        Ok(())
    }

    fn get_embedded_ids(&self) -> DomainResult<HashSet<i32>> {
        let conn = lock_conn(&self.conn)?;
        let mut stmt = conn.prepare("SELECT rowid FROM vec_bookmarks").map_err(|e| {
            DomainError::BookmarkOperationFailed(format!(
                "Failed to query embedded IDs: {}", e
            ))
        })?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i32>(0))
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect embedded IDs: {}", e
                ))
            })?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(|e| {
                DomainError::BookmarkOperationFailed(format!(
                    "Failed to collect embedded IDs: {}", e
                ))
            })?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_repo() -> SqliteVectorRepository {
        super::super::register_sqlite_vec();

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

        let results = repo
            .search_nearest(&[0.8f32, 0.8, 0.8, 0.8], 3)
            .unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 2);
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

        let ids = repo.get_embedded_ids().unwrap();
        assert_eq!(ids.len(), 1);
    }
}
