use diesel::{RunQueryDsl, SqliteConnection};
// src/infrastructure/repositories/sqlite/migration.rs
use crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError;
use diesel::sqlite::Sqlite;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{debug, instrument};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

/// Initializes the database by running all pending migrations.
///
/// This function takes a mutable reference to a `MigrationHarness` for a SQLite database.
/// It first reverts all migrations using the `revert_all_migrations` method.
/// Then, it retrieves all pending migrations and logs their names.
/// Finally, it runs all pending migrations using the `run_pending_migrations` method.
///
/// # Errors
///
/// This function will return an error if any of the following operations fail:
///
/// * Reverting all migrations
/// * Retrieving pending migrations
/// * Running pending migrations
#[allow(unused)]
pub fn init_db(
    connection: &mut impl MigrationHarness<Sqlite>,
) -> Result<(), SqliteRepositoryError> {
    debug!("{:?}", "--> initdb <--");

    connection.revert_all_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to revert migrations: {}", e))
    })?;

    let pending = connection.pending_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to get pending migrations: {}", e))
    })?;

    pending.iter().for_each(|m| {
        debug!("Pending Migration: {}", m.name());
    });

    connection.run_pending_migrations(MIGRATIONS).map_err(|e| {
        SqliteRepositoryError::MigrationError(format!("Failed to run pending migrations: {}", e))
    })?;

    Ok(())
}


/// Checks if the schema migrations table exists
#[instrument(skip(conn), level = "debug")]
pub fn check_schema_migrations_exists(
    conn: &mut SqliteConnection,
) -> Result<bool, SqliteRepositoryError> {
    use diesel::sql_query;
    use diesel::sql_types::Integer;
    use diesel::QueryableByName;

    #[derive(QueryableByName, Debug)]
    struct TableCheckResult {
        #[diesel(sql_type = Integer)]
        pub table_exists: i32,
    }

    let query = "
        SELECT COUNT(*) as table_exists
        FROM sqlite_master
        WHERE type='table' AND name='__diesel_schema_migrations'
    ";

    let result: TableCheckResult = sql_query(query)
        .get_result(conn)
        .map_err(SqliteRepositoryError::DatabaseError)?;

    Ok(result.table_exists > 0)
}
