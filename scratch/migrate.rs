use diesel::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use bkmr::dal::establish_connection;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

// embed_migrations!("./migrations");

fn main() {
    let conn = &mut establish_connection();
    run_migration(conn);
}

fn run_migration(conn: &mut SqliteConnection) {
    conn.run_pending_migrations(MIGRATIONS).unwrap();
    // conn.revert_last_migration(MIGRATIONS).unwrap();
}
