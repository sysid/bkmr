use super::error::{SqliteRepositoryError, SqliteResult};
use crate::app_state::AppState;
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
    run_pending_migrations(&pool)?;

    info!("Connection pool initialized successfully");
    Ok(pool)
}

/// Run any pending database migrations
#[instrument(level = "info")]
pub fn run_pending_migrations(pool: &ConnectionPool) -> SqliteResult<()> {
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

    // Get the database path from the pool connection
    let app_state = AppState::read_global();
    let db_path = &app_state.settings.db_url;
    let db_path = Path::new(db_path);

    // Only create a backup if the database file already exists
    if db_path.exists() {
        // Create backup with date suffix
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
        debug!("No existing database to backup before migrations");
    }

    // Run the migrations
    conn.run_pending_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to run migrations: {}", e))
    })?;

    eprintln!("Migrations completed successfully.");
    Ok(())
}
