use super::error::{SqliteRepositoryError, SqliteResult};
use crate::infrastructure::repositories::sqlite::migration::MIGRATIONS;
use chrono::Local;
use diesel::connection::SimpleConnection;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use std::fs;
use std::path::Path;
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
            )).with_source(e)
        })?;
        bootstrap
            .execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(|e| {
                SqliteRepositoryError::ConnectionPoolError(format!(
                    "Failed to set WAL journal mode: {}",
                    e
                )).with_source(e)
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

/// Check whether the native database has bookmark records before backup.
/// Only observed absence or a successful zero count is healthy emptiness.
/// The private native pool has no attached-database route: inspect its main and
/// temporary schemas, then retain SQLite's normal temporary-before-main COUNT
/// resolution, including case variants and views. This is not an observation
/// of arbitrary externally attached schemas or an atomic schema/count snapshot.
fn is_database_empty_for_backup(conn: &mut SqliteConnection) -> SqliteResult<bool> {
    use diesel::prelude::*;
    use diesel::sql_types::Integer;

    #[derive(QueryableByName, Debug)]
    struct BookmarkRelation {
        #[diesel(sql_type = Integer)]
        relation_exists: i32,
    }

    #[derive(QueryableByName, Debug)]
    struct BookmarkCount {
        #[diesel(sql_type = Integer)]
        count: i32,
    }

    let relation = diesel::sql_query(
        "SELECT EXISTS(SELECT 1 FROM sqlite_temp_master \
         WHERE type IN ('table', 'view') AND name = 'bookmarks' COLLATE NOCASE) \
         OR EXISTS(SELECT 1 FROM sqlite_master \
         WHERE type IN ('table', 'view') AND name = 'bookmarks' COLLATE NOCASE) \
         AS relation_exists",
    )
    .get_result::<BookmarkRelation>(conn)
    .map_err(|error| {
        SqliteRepositoryError::OperationFailed(
            "Failed to inspect bookmarks schema before backup".to_string(),
        )
        .with_source(error)
    })?;

    if relation.relation_exists == 0 {
        debug!("Native bookmarks relation is absent before backup");
        return Ok(true);
    }

    let bookmark_count = diesel::sql_query("SELECT COUNT(*) as count FROM bookmarks")
        .get_result::<BookmarkCount>(conn)
        .map_err(|error| {
            SqliteRepositoryError::OperationFailed(
                "Failed to count bookmarks before backup".to_string(),
            )
            .with_source(error)
        })?;
    debug!("Database contains {} bookmark records", bookmark_count.count);
    Ok(bookmark_count.count == 0)
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
            // Additional check: verify the database actually has user data
            let is_empty = is_database_empty_for_backup(&mut conn)?;

            if !is_empty {
                // Create backup with date suffix for non-empty databases
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

                    // Copy the database file and fail if backup creation fails
                    fs::copy(db_path, &backup_path).map_err(|e| {
                        SqliteRepositoryError::IoError(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to create backup: {}", e),
                        ))
                    })?;

                    eprintln!("Backup created at: {}", backup_path.display());
                } else {
                    return Err(SqliteRepositoryError::OperationFailed(
                        "Could not determine database filename for backup".to_string(),
                    ));
                }
            } else {
                debug!("Skipping backup for database with no user data");
            }
        } else {
            debug!("Skipping backup for small/empty database file");
        }
    } else {
        debug!("No existing database to backup before migrations");
    }

    // Run the migrations
    conn.run_pending_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to run migrations: {}", e))
    })?;

    info!("Migrations completed successfully");
    eprintln!("Migrations completed successfully.");
    Ok(())
}

#[cfg(test)]
mod backup_observation_tests {
    use super::*;
    use crate::domain::error::{DomainError, RepositoryError};
    use diesel::Connection;
    use rusqlite::{params, Connection as NativeSqlite, OpenFlags};
    use std::error::Error;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;

    struct NativeRun {
        owned: Option<TempDir>,
        db: PathBuf,
    }

    impl NativeRun {
        fn new() -> Self {
            let requested = match std::env::var_os("BKMR_NATIVE_TEST_ROOT") {
                Some(root) => {
                    let root = PathBuf::from(root);
                    assert!(root.is_absolute(), "native test Run must be absolute");
                    root
                }
                None => {
                    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../ProjectCentral/now/tmp");
                    fs::create_dir_all(&root).expect("product-local test Run space");
                    root
                }
            };
            let base = requested.canonicalize().expect("actual native test Run root");
            assert!(base.is_dir(), "native test Run must be an existing directory");
            let owned = tempfile::Builder::new()
                .prefix("bkmr-backup-observation-")
                .tempdir_in(base)
                .expect("exclusive native fixture child");
            let db = owned.path().join("native.db");
            Self { owned: Some(owned), db }
        }

        fn url(&self) -> &str {
            self.db.to_str().expect("actual fixture database UTF-8 coordinate")
        }

        fn diesel(&self) -> SqliteConnection {
            let mut conn = SqliteConnection::establish(self.url())
                .expect("real owned Diesel SQLite connection");
            conn.batch_execute("PRAGMA busy_timeout = 50; PRAGMA journal_mode = DELETE;")
                .expect("finite native busy timeout and committed main-file fixture");
            conn
        }

        fn sqlite(&self) -> NativeSqlite {
            let conn = NativeSqlite::open(&self.db).expect("real owned SQLite connection");
            conn.busy_timeout(Duration::from_millis(50)).unwrap();
            conn
        }

        fn backups(&self) -> Vec<PathBuf> {
            let mut result: Vec<_> = fs::read_dir(self.owned.as_ref().unwrap().path())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with("native_backup_"))
                .collect();
            result.sort();
            result
        }
    }

    impl Drop for NativeRun {
        fn drop(&mut self) {
            if let Some(owned) = self.owned.take() {
                if std::thread::panicking() {
                    let path = owned.keep();
                    eprintln!("retained failed native backup fixture {}", path.display());
                } else {
                    let path = owned.path().to_path_buf();
                    if let Err(error) = owned.close() {
                        panic!("owned fixture cleanup failed at {}: {error}", path.display());
                    }
                }
            }
        }
    }

    fn original_diesel(error: &(dyn Error + 'static)) -> &diesel::result::Error {
        let mut current = Some(error);
        while let Some(cause) = current {
            if let Some(original) = cause.downcast_ref::<diesel::result::Error>() {
                return original;
            }
            current = cause.source();
        }
        panic!("actual native Diesel source is missing");
    }

    type SchemaRow = (String, String, Option<String>);

    fn schema(conn: &NativeSqlite) -> Vec<SchemaRow> {
        conn.prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn versions(conn: &NativeSqlite) -> Vec<String> {
        conn.prepare("SELECT version FROM __diesel_schema_migrations ORDER BY version")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn native_row(conn: &NativeSqlite, table: &str) -> (i64, String, Option<Vec<u8>>) {
        // Only fixed fixture-owned identifiers are supplied by these tests.
        conn.query_row(
            &format!("SELECT id, metadata, embedding FROM {table}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    #[test]
    fn given_genuinely_absent_bookmark_relation_when_backup_preflight_then_absence_is_healthy_and_native_creation_reopens() {
        let run = NativeRun::new();
        let mut conn = run.diesel();
        let observer = run.sqlite();
        assert!(schema(&observer).is_empty(), "actual healthy schema has no relation");
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        conn.run_pending_migrations(MIGRATIONS).expect("actual embedded native schema");
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        drop(observer);
        drop(conn);
        let mut reopened = run.diesel();
        assert!(is_database_empty_for_backup(&mut reopened).unwrap());
        assert_eq!(run.sqlite().query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn given_empty_and_populated_native_bookmarks_when_backup_preflight_then_exact_count_controls_backup() {
        let run = NativeRun::new();
        let mut conn = run.diesel();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        let observer = run.sqlite();
        let before_schema = schema(&observer);
        let metadata = r#"{"fixture_extension":{"retained":true}}"#;
        let id: i64 = observer.query_row(
            "INSERT INTO bookmarks (URL, metadata) VALUES (?1, ?2) RETURNING id",
            params!["https://backup-observation.invalid/retained", metadata],
            |row| row.get(0),
        ).unwrap();
        assert!(id > 0, "native allocated ID was actually returned");
        let row = native_row(&observer, "bookmarks");
        assert_eq!(row, (id, metadata.to_string(), None));
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
        assert_eq!(schema(&observer), before_schema);
        assert_eq!(native_row(&observer, "bookmarks"), row);
        drop(observer);
        drop(conn);
        let mut reopened = run.diesel();
        assert!(!is_database_empty_for_backup(&mut reopened).unwrap());
        assert_eq!(native_row(&run.sqlite(), "bookmarks"), row);
    }

    #[test]
    fn given_actual_sqlite_schema_lock_when_backup_preflight_then_original_diesel_failure_is_unavailable_not_empty() {
        let run = NativeRun::new();
        let mut conn = run.diesel();
        conn.batch_execute("CREATE TABLE bookmarks (id INTEGER PRIMARY KEY);").unwrap();
        let observer = run.sqlite();
        let locker = run.sqlite();
        locker.execute_batch("BEGIN EXCLUSIVE;").expect("actual SQLite exclusive lock");
        let oracle = observer.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0)).unwrap_err();
        match oracle {
            rusqlite::Error::SqliteFailure(code, _) => assert_eq!(code.code, rusqlite::ErrorCode::DatabaseBusy),
            other => panic!("wrong actual SQLite lock prerequisite: {other}"),
        }
        let error = is_database_empty_for_backup(&mut conn).expect_err("real schema lock is unavailable, not empty");
        assert!(matches!(original_diesel(&error), diesel::result::Error::DatabaseError(..)));
        assert!(matches!(error.presentation(), SqliteRepositoryError::OperationFailed(message) if message == "Failed to inspect bookmarks schema before backup"));
        locker.execute_batch("ROLLBACK;").expect("release actual owned lock");
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        observer.execute("INSERT INTO bookmarks DEFAULT VALUES", []).unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
    }

    #[test]
    fn given_real_broken_bookmark_view_when_backup_preflight_then_failed_count_is_not_absence_and_repair_reopens() {
        let run = NativeRun::new();
        let mut conn = run.diesel();
        conn.batch_execute("CREATE VIEW bookmarks AS SELECT * FROM missing_owner_source;").unwrap();
        let observer = run.sqlite();
        let before_schema = schema(&observer);
        assert_eq!(observer.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='view' AND name='bookmarks'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        assert!(matches!(observer.query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get::<_, i64>(0)).unwrap_err(), rusqlite::Error::SqliteFailure(..)));
        let error = is_database_empty_for_backup(&mut conn).expect_err("real failed COUNT must not authorize empty");
        let original = original_diesel(&error) as *const diesel::result::Error;
        assert!(matches!(error.presentation(), SqliteRepositoryError::OperationFailed(message) if message == "Failed to count bookmarks before backup"));
        let contextual = error.context("native backup preflight");
        assert_eq!(original_diesel(&contextual) as *const _, original);
        let repository: RepositoryError = contextual.into();
        assert_eq!(original_diesel(&repository) as *const _, original);
        let domain = DomainError::from(repository).context("native migration admission");
        assert_eq!(original_diesel(&domain) as *const _, original);
        assert_eq!(schema(&observer), before_schema, "failed observation never repairs schema");
        conn.batch_execute("DROP VIEW bookmarks; CREATE TABLE bookmarks (id INTEGER PRIMARY KEY); INSERT INTO bookmarks DEFAULT VALUES;").unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
        drop(observer);
        drop(conn);
        assert!(!is_database_empty_for_backup(&mut run.diesel()).unwrap());
    }

    #[test]
    fn given_working_case_and_view_bookmark_relations_when_backup_preflight_then_existing_count_compatibility_survives() {
        let run = NativeRun::new();
        let mut conn = run.diesel();
        conn.batch_execute("CREATE TABLE Bookmarks (id INTEGER PRIMARY KEY);").unwrap();
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("INSERT INTO Bookmarks DEFAULT VALUES;").unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("DROP TABLE Bookmarks; CREATE TABLE backing (id INTEGER PRIMARY KEY); CREATE VIEW bookmarks AS SELECT * FROM backing;").unwrap();
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("INSERT INTO backing DEFAULT VALUES;").unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("CREATE TEMP TABLE Bookmarks (id INTEGER PRIMARY KEY);").unwrap();
        assert!(is_database_empty_for_backup(&mut conn).unwrap(), "real temporary empty table shadows populated main view");
        conn.batch_execute("INSERT INTO temp.Bookmarks DEFAULT VALUES;").unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("DROP TABLE temp.Bookmarks; DROP VIEW main.bookmarks;").unwrap();
        assert!(is_database_empty_for_backup(&mut conn).unwrap());
        conn.batch_execute("CREATE TEMP VIEW bookmarks AS SELECT * FROM main.backing;").unwrap();
        assert!(!is_database_empty_for_backup(&mut conn).unwrap(), "temporary-only real view is not absent");
    }

    #[test]
    fn given_pending_native_migration_and_real_failed_count_when_migrating_then_failure_precedes_backup_and_mutation() {
        let run = NativeRun::new();
        let mut setup = run.diesel();
        let pending = setup.pending_migrations(MIGRATIONS).unwrap();
        assert!(pending.len() > 1, "genuine embedded prefix prerequisite");
        assert_eq!(pending.last().unwrap().name().version().to_string(), "20260404100000", "actual final embedding-clearing migration");
        setup.run_migrations(&pending[..pending.len() - 1]).unwrap();
        let remaining = setup.pending_migrations(MIGRATIONS).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name().version().to_string(), "20260404100000");
        drop(setup);
        let observer = run.sqlite();
        let metadata = "owned-native-retained-".repeat(4096);
        let blob = vec![7_u8, 13, 29, 41];
        let id: i64 = observer.query_row(
            "INSERT INTO bookmarks (URL, metadata, embedding) VALUES (?1, ?2, ?3) RETURNING id",
            params!["https://backup-observation.invalid/migration", metadata, blob],
            |row| row.get(0),
        ).unwrap();
        assert!(id > 0);
        let retained = native_row(&observer, "bookmarks");
        assert_eq!(retained, (id, metadata, Some(blob)));
        observer.execute_batch("ALTER TABLE bookmarks RENAME TO retained_bookmarks; CREATE VIEW bookmarks AS SELECT * FROM missing_owner_source;").unwrap();
        let journal: String = observer.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0)).unwrap();
        assert_eq!(journal, "delete");
        let before_schema = schema(&observer);
        let before_versions = versions(&observer);
        let before_bytes = fs::read(&run.db).unwrap();
        assert!(before_bytes.len() >= 16384, "actual unchanged size gate is reached");
        assert!(run.backups().is_empty());
        // The native pool is real, but init_pool would replay the target before
        // the test. Use the existing pool type directly with bounded admission.
        let manager = ConnectionManager::<SqliteConnection>::new(run.url());
        let pool = ConnectionPool::builder().max_size(1)
            .connection_timeout(Duration::from_secs(2)).build(manager).unwrap();
        let error = run_pending_migrations(&pool, run.url()).expect_err("failed native COUNT must stop before backup/migration");
        assert!(matches!(original_diesel(&error), diesel::result::Error::DatabaseError(..)));
        assert!(matches!(error.presentation(), SqliteRepositoryError::OperationFailed(message) if message == "Failed to count bookmarks before backup"));
        assert_eq!(schema(&observer), before_schema);
        assert_eq!(versions(&observer), before_versions);
        assert_eq!(native_row(&observer, "retained_bookmarks"), retained);
        assert_eq!(fs::read(&run.db).unwrap(), before_bytes);
        assert!(run.backups().is_empty(), "wrong phase cannot qualify");
        observer.execute_batch("DROP VIEW bookmarks; ALTER TABLE retained_bookmarks RENAME TO bookmarks;").unwrap();
        // Affirm committed DELETE mode AFTER the last restoration. The copied
        // main file is an owned fixture oracle, not a concurrent WAL guarantee.
        let journal: String = observer.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0)).unwrap();
        assert_eq!(journal, "delete");
        assert_eq!(native_row(&observer, "bookmarks"), retained);
        drop(observer);
        run_pending_migrations(&pool, run.url()).expect("same native operation after actual restoration");
        drop(pool);
        let backups = run.backups();
        assert_eq!(backups.len(), 1, "actual native backup was published");
        let backup = NativeSqlite::open_with_flags(&backups[0], OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert_eq!(native_row(&backup, "bookmarks"), retained);
        assert_eq!(versions(&backup), before_versions, "backup precedes final migration");
        let current = run.sqlite();
        assert_eq!(native_row(&current, "bookmarks"), (retained.0, retained.1.clone(), None));
        let mut final_versions = before_versions;
        final_versions.push("20260404100000".to_string());
        assert_eq!(versions(&current), final_versions);
        drop(current);
        drop(backup);
        let mut reopened = run.diesel();
        assert!(reopened.pending_migrations(MIGRATIONS).unwrap().is_empty());
        assert!(!is_database_empty_for_backup(&mut reopened).unwrap());
        assert_eq!(run.backups(), backups, "read/reopen did not invent another backup");
    }
}
