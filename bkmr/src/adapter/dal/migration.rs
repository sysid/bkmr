use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use diesel::sqlite::Sqlite;
use tracing::debug;

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
// In helper.rs
pub fn init_db(
    connection: &mut impl MigrationHarness<Sqlite>,
) -> anyhow::Result<()> {
    debug!("{:?}", "--> initdb <--");

    connection.revert_all_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to revert migrations: {}", e))?;

    let pending = connection
        .pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to get pending migrations: {}", e))?;

    pending.iter().for_each(|m| {
        debug!(
            "Pending Migration: {}",
            m.name()
        );
    });

    connection.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Failed to run pending migrations: {}", e))?;

    Ok(())
}