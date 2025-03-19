use super::error::{SqliteRepositoryError, SqliteResult};
use crate::infrastructure::repositories::sqlite::migration::MIGRATIONS;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use diesel::Connection;
use diesel::RunQueryDsl;
use diesel_migrations::MigrationHarness;
use std::path::Path;
use tracing::{debug, info, trace};

pub type ConnectionPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;

/// Establish a new database connection without using anyhow
pub fn establish_connection(database_url: &str) -> SqliteResult<SqliteConnection> {
    debug!("Establishing connection to: {}", database_url);

    let conn = diesel::sqlite::SqliteConnection::establish(database_url)
        .map_err(SqliteRepositoryError::ConnectionError)?;

    Ok(conn)
}

/// Initialize a connection pool (no anyhow)
pub fn init_pool(database_url: &str) -> SqliteResult<ConnectionPool> {
    debug!("Initializing connection pool for: {}", database_url);

    // Create parent directory if it doesn't exist
    if let Some(parent) = Path::new(database_url).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(SqliteRepositoryError::IoError)?;
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
pub fn run_pending_migrations(pool: &ConnectionPool) -> SqliteResult<()> {
    let mut conn = pool
        .get()
        .map_err(|e| SqliteRepositoryError::ConnectionPoolError(e.to_string()))?;

    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| SqliteRepositoryError::MigrationError(e.to_string()))?;

    Ok(())
}

/// Check if the database has the required embedding column
pub fn check_embedding_column_exists(conn: &mut SqliteConnection) -> SqliteResult<bool> {
    use diesel::sql_query;
    use diesel::sql_types::Integer;
    use diesel::QueryableByName;
    use tracing::trace;

    #[derive(QueryableByName, Debug)]
    struct ColumnCheck {
        #[diesel(sql_type = Integer)]
        column_exists: i32,
    }

    let query = "
    SELECT COUNT(*) as column_exists
    FROM pragma_table_info('bookmarks')
    WHERE name='embedding';
    ";

    let result: Vec<ColumnCheck> = sql_query(query)
        .load(conn)
        .map_err(SqliteRepositoryError::DatabaseError)?;

    trace!("Embedding ColumnCheck: {:?}", result);
    Ok(result.iter().any(|item| item.column_exists > 0))
}
