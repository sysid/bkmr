use super::error::{SqliteRepositoryError, SqliteResult};
use crate::infrastructure::repositories::sqlite::migration::MIGRATIONS;
use chrono::Local;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use std::fs;
use std::path::Path;
use tracing::{debug, info, instrument};

pub type ConnectionPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;

/// Initialize a connection pool
pub fn init_pool(database_url: &str) -> SqliteResult<ConnectionPool> {
    debug!("Initializing connection pool for: {}", database_url);

    // Create parent directory if it doesn't exist
    if let Some(parent) = Path::new(database_url).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(SqliteRepositoryError::IoError)?;
        }
    }

    // Build the pool
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(15)
        .build(manager)
        .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))?;

    // Run migrations
    run_pending_migrations(&pool, database_url)?;

    info!("Connection pool initialized successfully");
    Ok(pool)
}

/// Check if the database has meaningful user data
/// A database is considered empty for backup purposes if it has no bookmark records
fn is_database_empty_for_backup(conn: &mut SqliteConnection) -> SqliteResult<bool> {
    use diesel::prelude::*;
    use diesel::sql_types::Integer;
    
    #[derive(QueryableByName, Debug)]
    struct BookmarkCount {
        #[diesel(sql_type = Integer)]
        count: i32,
    }
    
    // Check if the bookmarks table exists and has data
    // If it doesn't exist or has no records, consider it empty for backup purposes
    let result: Result<BookmarkCount, diesel::result::Error> = diesel::sql_query(
        "SELECT COUNT(*) as count FROM bookmarks"
    )
    .get_result::<BookmarkCount>(conn);
    
    match result {
        Ok(bookmark_count) => {
            debug!("Database contains {} bookmark records", bookmark_count.count);
            Ok(bookmark_count.count == 0)
        },
        Err(e) => {
            // If bookmarks table doesn't exist, definitely empty for backup purposes
            debug!("Bookmarks table doesn't exist or query failed: {}", e);
            Ok(true)
        }
    }
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
    eprintln!("This version requires DB schema migration:");
    for migration in &pending {
        eprintln!("  - {}", migration.name());
    }

    // Get the database path from the parameter
    let db_path = Path::new(database_url);

    // Only create a backup if the database file exists and has meaningful size
    if db_path.exists() {
        // Check file size first - a newly created empty SQLite database is typically < 4KB
        let file_metadata = fs::metadata(db_path).map_err(|e| {
            SqliteRepositoryError::IoError(e)
        })?;
        
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

    eprintln!("Migrations completed successfully.");
    Ok(())
}
