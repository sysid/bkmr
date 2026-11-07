use super::error::{SqliteRepositoryError, SqliteResult};
use crate::infrastructure::repositories::sqlite::migration::MIGRATIONS;
use chrono::Local;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, info, instrument};

pub type ConnectionPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;

/// Sets busy_timeout on every pool connection so concurrent access retries
/// instead of failing immediately with SQLITE_BUSY.
/// WAL mode is set once in init_pool before the pool is built — it's a
/// file-level property that persists, so per-connection setting is unnecessary.
#[derive(Debug)]
struct SqliteBusyTimeoutCustomizer;

impl r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for SqliteBusyTimeoutCustomizer
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA busy_timeout = 5000;")
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

/// Initialize a connection pool with consistent SQLite pragmas.
///
/// Strategy: WAL journal mode is set once via a bootstrap connection before
/// pool creation. busy_timeout is set per-connection via on_acquire. This
/// ensures all connections (diesel pool and rusqlite) use the same journal
/// mode, eliminating mode-mismatch contention.
pub fn init_pool(database_url: &str) -> SqliteResult<ConnectionPool> {
    debug!("Initializing connection pool for: {}", database_url);

    // Create parent directory if it doesn't exist
    if let Some(parent) = Path::new(database_url).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(SqliteRepositoryError::IoError)?;
        }
    }

    // Set WAL mode once before any pool connections open. WAL is a file-level
    // property that persists — all subsequent connections inherit it.
    // This avoids the journal_mode=delete vs WAL mismatch between the diesel
    // pool and the rusqlite connection in SqliteVectorRepository.
    {
        let bootstrap = rusqlite::Connection::open(database_url).map_err(|e| {
            SqliteRepositoryError::ConnectionPoolError(format!(
                "Failed to open bootstrap connection for WAL setup: {}",
                e
            ))
        })?;
        bootstrap
            .execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(|e| {
                SqliteRepositoryError::ConnectionPoolError(format!(
                    "Failed to set WAL journal mode: {}",
                    e
                ))
            })?;
        debug!("WAL journal mode set via bootstrap connection");
        // bootstrap connection drops here — WAL mode persists on the file
    }

    // Build the pool. Pool size 4 is sufficient for a CLI tool.
    // on_acquire sets busy_timeout=5000 on each connection.
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(4)
        .connection_customizer(Box::new(SqliteBusyTimeoutCustomizer))
        .build(manager)
        .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))?;

    // Run migrations
    run_pending_migrations(&pool, database_url)?;

    info!("Connection pool initialized successfully");
    Ok(pool)
}

/// Run any pending database migrations
#[instrument(level = "info")]
pub fn run_pending_migrations(pool: &ConnectionPool, database_url: &str) -> SqliteResult<()> {
    let mut conn = pool
        .get()
        .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))?;

    // Check if there are pending migrations before prompting
    let pending = conn.pending_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to check pending migrations: {}", e))
    })?;

    if pending.is_empty() {
        debug!("No pending migrations to run");
        return Ok(());
    }

    // Display pending migrations
    info!(count = pending.len(), "Running pending database migrations");
    eprintln!("This version requires DB schema migration:");
    for migration in &pending {
        eprintln!("  - {}", migration.name());
    }

    // Get the database path from the parameter
    let db_path = Path::new(database_url);

    // Only create a backup if the database file exists and has meaningful size
    if db_path.exists() {
        // Check file size first - a newly created empty SQLite database is typically < 4KB
        let file_metadata = fs::metadata(db_path).map_err(|e| SqliteRepositoryError::IoError(e))?;

        let file_size = file_metadata.len();

        // A meaningful database with user data is typically much larger
        // Fresh databases with just migration metadata are relatively small
        let is_likely_empty = file_size < 16384; // 16KB threshold

        if !is_likely_empty {
            // ponytail: size gate only, no bookmark row count. A count that fails
            // (lock, corruption) must never be read as "empty" and skip the backup;
            // backing up a large-but-empty DB is harmless.
            let date_suffix = Local::now().format("%Y%m%d").to_string();

            if let Some(file_name) = db_path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                let backup_name = if let Some(ext_pos) = file_name_str.rfind('.') {
                    let (name, ext) = file_name_str.split_at(ext_pos);
                    format!("{}_backup_{}{}", name, date_suffix, ext)
                } else {
                    format!("{}_backup_{}", file_name_str, date_suffix)
                };

                let backup_path = db_path.with_file_name(backup_name);

                // Not a file copy: commits can still be in the log while another process holds
                // the database open. Fail if backup creation fails.
                backup_database(&mut conn, &backup_path)?;

                eprintln!("Backup created at: {}", backup_path.display());
            } else {
                return Err(SqliteRepositoryError::OperationFailed(
                    "Could not determine database filename for backup".to_string(),
                ));
            }
        } else {
            debug!("Skipping backup for small/empty database file");
        }
    } else {
        debug!("No existing database to backup before migrations");
    }

    // Holds the write lock from before the pending check inside run_pending_migrations. Otherwise
    // processes starting together all see the migration as pending, and all but the first fail.
    conn.immediate_transaction::<_, SqliteRepositoryError, _>(|conn| {
        conn.run_pending_migrations(MIGRATIONS).map_err(|e| {
            SqliteRepositoryError::MigrationError(format!("Failed to run migrations: {}", e))
        })?;
        Ok(())
    })?;

    info!("Migrations completed successfully");
    eprintln!("Migrations completed successfully.");
    Ok(())
}

/// A consistent copy of the database including the log. Replaces a backup of the same day, and goes
/// through a temporary file because VACUUM INTO refuses an existing file and because processes can
/// start together.
fn backup_database(conn: &mut SqliteConnection, backup_path: &Path) -> SqliteResult<()> {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let temp_path = backup_path.with_extension(format!(
        "tmp{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let quoted = temp_path.to_string_lossy().replace('\'', "''");

    conn.batch_execute(&format!("VACUUM INTO '{}'", quoted))
        .map_err(|e| {
            SqliteRepositoryError::OperationFailed(format!("Failed to create backup: {}", e))
        })?;
    fs::rename(&temp_path, backup_path).map_err(|e| {
        SqliteRepositoryError::OperationFailed(format!("Failed to create backup: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::Connection;

    fn backups_in(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("_backup_"))
            .collect()
    }

    fn pool_for(url: &str) -> ConnectionPool {
        r2d2::Pool::builder()
            .max_size(1)
            .build(ConnectionManager::<SqliteConnection>::new(url))
            .unwrap()
    }

    #[test]
    fn given_history_migration_when_reverted_then_memories_can_be_written_and_it_applies_again() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        conn.batch_execute(
            "INSERT INTO bookmarks (URL, metadata, tags, desc, flags)
             VALUES ('memory', 'title', ',_mem_,', '', 0)",
        )
        .unwrap();

        revert_through_history_migration(&mut conn);

        // A leftover trigger would fail these writes: its table is gone
        conn.batch_execute("UPDATE bookmarks SET URL = 'changed'; DELETE FROM bookmarks;")
            .unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
    }

    /// Later migrations are reverted too, so that this keeps reaching the history migration.
    fn revert_through_history_migration(conn: &mut SqliteConnection) {
        loop {
            let reverted = conn.revert_last_migration(MIGRATIONS).unwrap().to_string();
            if reverted == "20261107100000" {
                return;
            }
        }
    }

    /// A user database one migration behind, large enough to be backed up before migrating.
    fn create_db_before_history_migration(db: &Path) {
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        revert_through_history_migration(&mut conn);
        let padding = "x".repeat(2000);
        for id in [1, 5, 9, 10, 11, 12, 13, 14, 15, 16] {
            conn.batch_execute(&format!(
                "INSERT INTO bookmarks (id, URL, metadata, tags, desc, flags)
                 VALUES ({id}, 'https://example.com/{id}', 'zqtitle{id}', ',plain,', '{padding}', 0)"
            ))
            .unwrap();
        }
    }

    #[test]
    fn given_existing_bookmarks_when_history_migration_runs_then_they_and_their_search_survive() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        create_db_before_history_migration(&db);

        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);

        let reader = rusqlite::Connection::open(&db).unwrap();
        let ids: Vec<i64> = reader
            .prepare("SELECT id FROM bookmarks ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(ids, vec![1, 5, 9, 10, 11, 12, 13, 14, 15, 16]);
        let found: i64 = reader
            .query_row(
                "SELECT rowid FROM bookmarks_fts WHERE bookmarks_fts MATCH 'zqtitle5'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 5);
    }

    #[test]
    fn given_old_index_with_id_entries_when_migrated_then_it_matches_the_rows_and_a_deleted_id_is_searchable(
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        create_db_before_history_migration(&db);
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        // An old index holds id entries, and the delete trigger leaves them behind
        conn.batch_execute(
            "INSERT INTO bookmarks_fts(bookmarks_fts) VALUES('rebuild');
             DELETE FROM bookmarks WHERE id = 16;",
        )
        .unwrap();

        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);

        let reader = rusqlite::Connection::open(&db).unwrap();
        // Rank 1 also compares the index with the rows
        reader
            .execute_batch(
                "INSERT INTO bookmarks_fts(bookmarks_fts, rank) VALUES('integrity-check', 1)",
            )
            .expect("index must match the rows");
        let deleted_id_hits: i64 = reader
            .query_row(
                "SELECT count(id) FROM bookmarks_fts WHERE bookmarks_fts MATCH '16'",
                [],
                |r| r.get(0),
            )
            .expect("searching a deleted ID must not fail");
        assert_eq!(deleted_id_hits, 0);
        let found: i64 = reader
            .query_row(
                "SELECT id FROM bookmarks_fts WHERE bookmarks_fts MATCH 'zqtitle5'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 5);
    }

    /// All columns of all rows, and each column's definition, to compare before and after.
    fn snapshot_of_bookmarks(
        conn: &rusqlite::Connection,
    ) -> (Vec<Vec<rusqlite::types::Value>>, Vec<String>) {
        let mut rows = conn.prepare("SELECT * FROM bookmarks ORDER BY id").unwrap();
        let columns = rows.column_count();
        let values = rows
            .query_map([], |row| {
                (0..columns)
                    .map(|index| row.get::<_, rusqlite::types::Value>(index))
                    .collect()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let definitions = conn
            .prepare(
                "SELECT cid || '|' || name || '|' || type || '|' || \"notnull\" || '|' ||
                        coalesce(dflt_value, 'NULL') || '|' || pk
                 FROM pragma_table_info('bookmarks')",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        (values, definitions)
    }

    fn fts_hits(conn: &rusqlite::Connection, term: &str) -> i64 {
        conn.query_row(
            "SELECT count(id) FROM bookmarks_fts WHERE bookmarks_fts MATCH ?",
            [term],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn assert_index_matches_rows(conn: &rusqlite::Connection) {
        // Rank 1 also compares the index with the rows
        conn.execute_batch(
            "INSERT INTO bookmarks_fts(bookmarks_fts, rank) VALUES('integrity-check', 1)",
        )
        .expect("index must match the rows");
    }

    #[test]
    fn given_row_with_every_column_set_when_migrated_then_all_values_and_definitions_are_unchanged()
    {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        create_db_before_history_migration(&db);
        let before = {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(
                "INSERT INTO bookmarks (id, URL, metadata, tags, desc, flags, last_update_ts,
                                        embedding, content_hash, created_ts, embeddable, file_path,
                                        file_mtime, file_hash, opener, accessed_at)
                 VALUES (77, 'https://example.com/full', 'title', ',a,b,', 'description', 3,
                         '2020-01-02 03:04:05', x'0102', x'0304', '2019-01-02 03:04:05', 1,
                         '/notes/full.md', 123, 'hash', 'open %s', '2021-01-02 03:04:05')",
            )
            .unwrap();
            snapshot_of_bookmarks(&conn)
        };

        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);

        let conn = rusqlite::Connection::open(&db).unwrap();
        assert_eq!(snapshot_of_bookmarks(&conn), before);
    }

    #[test]
    fn given_rows_from_before_the_migration_when_changed_then_every_trigger_fires() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        create_db_before_history_migration(&db);
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.batch_execute(
            "INSERT INTO bookmarks (id, URL, metadata, tags, desc, flags)
             VALUES (50, 'zqmemoryold', 'memory', ',_mem_,', '', 0)",
        )
        .unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);
        let conn = rusqlite::Connection::open(&db).unwrap();
        let history = |change: &str| -> i64 {
            conn.query_row(
                "SELECT count(*) FROM bookmark_history WHERE bookmark_id = 50 AND change = ?",
                [change],
                |r| r.get(0),
            )
            .unwrap()
        };
        let all_history = || -> i64 {
            conn.query_row("SELECT count(*) FROM bookmark_history", [], |r| r.get(0))
                .unwrap()
        };

        // bookmarks_ai
        conn.execute_batch(
            "INSERT INTO bookmarks (URL, metadata, tags, desc, flags)
             VALUES ('zqinserted', 'new', ',plain,', '', 0)",
        )
        .unwrap();
        assert_eq!(fts_hits(&conn, "zqinserted"), 1);

        // bookmarks_au, also for a row that is not a memory
        conn.execute_batch("UPDATE bookmarks SET metadata = 'zqplainnew' WHERE id = 5")
            .unwrap();
        assert_eq!(fts_hits(&conn, "zqtitle5"), 0);
        assert_eq!(fts_hits(&conn, "zqplainnew"), 1);
        assert_eq!(all_history(), 0, "only memories keep history");

        // bookmarks_au and bookmarks_history_au
        conn.execute_batch("UPDATE bookmarks SET URL = 'zqmemorynew' WHERE id = 50")
            .unwrap();
        assert_eq!(fts_hits(&conn, "zqmemoryold"), 0);
        assert_eq!(fts_hits(&conn, "zqmemorynew"), 1);
        assert_eq!(history("update"), 1);

        // bookmarks_ad and bookmarks_history_ad
        conn.execute_batch("DELETE FROM bookmarks WHERE id = 50")
            .unwrap();
        assert_eq!(fts_hits(&conn, "zqmemorynew"), 0);
        assert_eq!(history("delete"), 1);

        // bookmarks_ad for a row that is not a memory
        conn.execute_batch("DELETE FROM bookmarks WHERE id = 9")
            .unwrap();
        assert_eq!(fts_hits(&conn, "zqtitle9"), 0);
        assert_eq!(all_history(), 2, "only memories keep history");

        assert_index_matches_rows(&conn);
    }

    #[test]
    fn given_migration_failing_after_the_rebuild_when_run_then_the_database_stays_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap();
        create_db_before_history_migration(&db);
        // Fails CREATE TABLE bookmark_history, after bookmarks and its index were already replaced
        let before = {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch("CREATE TABLE bookmark_history (blocker INTEGER)")
                .unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT group_concat(sql) FROM sqlite_master WHERE name LIKE 'bookmarks%'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            (snapshot_of_bookmarks(&conn), schema)
        };

        assert!(run_pending_migrations(&pool_for(url), url).is_err());

        let conn = rusqlite::Connection::open(&db).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT group_concat(sql) FROM sqlite_master WHERE name LIKE 'bookmarks%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!((snapshot_of_bookmarks(&conn), schema), before);
        assert_eq!(fts_hits(&conn, "zqtitle5"), 1);
        let recorded: i64 = conn
            .query_row(
                "SELECT count(*) FROM __diesel_schema_migrations WHERE version = '20261107100000'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(recorded, 0, "a failed migration must run again next time");
    }

    #[test]
    fn given_databases_from_older_versions_when_migrated_then_rows_and_index_are_intact() {
        let dir = tempfile::tempdir().unwrap();

        for name in [
            "schema_v1_migration_test.db",
            "schema_v2_no_embeddings.db",
            "schema_v2_with_embeddings.db",
        ] {
            let old_db = dir.path().join(name);
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/resources")
                    .join(name),
                &old_db,
            )
            .unwrap();
            let rows_before = snapshot_of_bookmarks(&rusqlite::Connection::open(&old_db).unwrap())
                .0
                .len();
            assert!(rows_before > 0, "{name} is meant to hold bookmarks");

            let mut conn = SqliteConnection::establish(old_db.to_str().unwrap()).unwrap();
            conn.run_pending_migrations(MIGRATIONS).unwrap();
            drop(conn);

            let conn = rusqlite::Connection::open(&old_db).unwrap();
            assert_eq!(snapshot_of_bookmarks(&conn).0.len(), rows_before, "{name}");
            assert_index_matches_rows(&conn);
        }
    }

    #[test]
    fn given_migrated_db_when_the_highest_id_is_deleted_then_the_next_row_gets_a_new_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        create_db_before_history_migration(&db);
        let mut conn = SqliteConnection::establish(db.to_str().unwrap()).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);

        let writer = rusqlite::Connection::open(&db).unwrap();
        writer
            .execute_batch(
                "DELETE FROM bookmarks WHERE id = 16;
                 INSERT INTO bookmarks (URL, metadata, tags, desc, flags)
                 VALUES ('https://example.com/new', 'new', ',plain,', '', 0);",
            )
            .unwrap();
        let new_id: i64 = writer
            .query_row(
                "SELECT id FROM bookmarks WHERE URL = 'https://example.com/new'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_id, 17);
        let found: i64 = writer
            .query_row(
                "SELECT rowid FROM bookmarks_fts WHERE bookmarks_fts MATCH 'zqtitle5'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 5, "search triggers must survive the table rebuild");
    }

    #[test]
    fn given_commit_still_in_the_log_when_migrating_then_the_backup_contains_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap();
        create_db_before_history_migration(&db);

        // Stands in for an LSP server or a running agent, which keeps the log from being merged
        let holder = rusqlite::Connection::open(&db).unwrap();
        holder
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA wal_autocheckpoint = 0;
                 INSERT INTO bookmarks (URL, metadata, tags, desc, flags)
                 VALUES ('https://example.com/only-in-log', 't', ',plain,', '', 0);",
            )
            .unwrap();

        run_pending_migrations(&pool_for(url), url).unwrap();

        let backups = backups_in(dir.path());
        assert_eq!(backups.len(), 1, "{backups:?}");
        let backup = rusqlite::Connection::open(dir.path().join(&backups[0])).unwrap();
        let found: i64 = backup
            .query_row(
                "SELECT count(*) FROM bookmarks WHERE URL = 'https://example.com/only-in-log'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "the backup misses a committed bookmark");
    }

    #[test]
    fn given_backup_of_today_exists_when_migrating_again_then_it_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap();
        create_db_before_history_migration(&db);
        run_pending_migrations(&pool_for(url), url).unwrap();
        let mut conn = SqliteConnection::establish(url).unwrap();
        revert_through_history_migration(&mut conn);
        drop(conn);

        run_pending_migrations(&pool_for(url), url).unwrap();

        assert_eq!(backups_in(dir.path()).len(), 1);
    }

    #[test]
    fn given_concurrent_first_runs_behind_a_writer_when_migrating_then_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap().to_string();
        create_db_before_history_migration(&db);

        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let writer_url = url.clone();
        let writer = std::thread::spawn(move || {
            let conn = rusqlite::Connection::open(writer_url).unwrap();
            conn.execute_batch("PRAGMA journal_mode = WAL; BEGIN IMMEDIATE;")
                .unwrap();
            locked_tx.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(1));
            conn.execute_batch("COMMIT").unwrap();
        });
        locked_rx.recv().unwrap();

        let runs: Vec<_> = (0..4)
            .map(|_| {
                let url = url.clone();
                std::thread::spawn(move || init_pool(&url).map(|_| ()))
            })
            .collect();

        let results: Vec<_> = runs.into_iter().map(|run| run.join().unwrap()).collect();
        writer.join().unwrap();
        for result in results {
            assert!(result.is_ok(), "{result:?}");
        }
    }

    #[test]
    fn given_db_over_16kb_with_failing_bookmarks_count_when_migrating_then_backup_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap();

        // A real user database one migration behind, whose bookmarks relation
        // can no longer be counted (stand-in for corruption or lock failure).
        let mut conn = SqliteConnection::establish(url).unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        revert_through_history_migration(&mut conn);
        conn.batch_execute(
            "DROP TABLE bookmarks;
             CREATE TABLE gone (x INTEGER);
             CREATE VIEW bookmarks AS SELECT * FROM gone;
             DROP TABLE gone;",
        )
        .unwrap();
        assert!(
            conn.batch_execute("SELECT COUNT(*) FROM bookmarks")
                .is_err(),
            "fixture must make the bookmarks count fail"
        );
        drop(conn);
        assert!(
            fs::metadata(&db).unwrap().len() >= 16384,
            "fixture must exceed size gate"
        );

        // The migration itself may fail on the broken schema; the backup must exist regardless.
        let _ = run_pending_migrations(&pool_for(url), url);

        assert_eq!(
            backups_in(dir.path()).len(),
            1,
            "user DB must be backed up before migrating"
        );
    }

    #[test]
    fn given_db_under_16kb_when_migrating_then_no_backup_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("bkmr.db");
        let url = db.to_str().unwrap();
        fs::File::create(&db).unwrap();

        run_pending_migrations(&pool_for(url), url).unwrap();

        assert!(backups_in(dir.path()).is_empty());
    }
}
