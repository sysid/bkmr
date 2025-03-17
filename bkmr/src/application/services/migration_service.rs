// src/application/services/migration_service.rs

use crate::application::error::{ApplicationError, ApplicationResult};
use diesel::connection::SimpleConnection;
use diesel_migrations::MigrationHarness;
use tracing::{debug, info};

pub struct MigrationService {
    repository_url: String,
}

impl MigrationService {
    pub fn new(repository_url: String) -> Self {
        Self { repository_url }
    }

    pub fn check_embedding_column_exists(&self) -> ApplicationResult<bool> {
        use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
        use crate::infrastructure::repositories::sqlite::connection::check_embedding_column_exists;

        // Create repository
        let repository = SqliteBookmarkRepository::from_url(&self.repository_url)
            .map_err(|e| ApplicationError::Other(format!("Failed to create repository: {}", e)))?;

        // Get a connection
        let mut conn = repository
            .get_connection()
            .map_err(|e| ApplicationError::Other(format!("Failed to get connection: {}", e)))?;

        // Check if embedding column exists
        check_embedding_column_exists(&mut conn).map_err(|e| {
            ApplicationError::Other(format!("Failed to check embedding column: {}", e))
        })
    }

    pub fn run_migrations(&self) -> ApplicationResult<()> {
        use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
        use crate::infrastructure::repositories::sqlite::migration::MIGRATIONS;

        // Create repository
        let repository = SqliteBookmarkRepository::from_url(&self.repository_url)
            .map_err(|e| ApplicationError::Other(format!("Failed to create repository: {}", e)))?;

        // Get a connection
        let mut conn = repository
            .get_connection()
            .map_err(|e| ApplicationError::Other(format!("Failed to get connection: {}", e)))?;

        // Check if migrations table exists
        let migrations_exist = self
            .check_if_migrations_table_exists(&mut conn)
            .map_err(|e| ApplicationError::Other(e.to_string()))?;

        // Create migrations table if it doesn't exist
        if !migrations_exist {
            const MIGRATION_TABLE_SQL: &str = r#"
            BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY NOT NULL,
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO __diesel_schema_migrations (version, run_on)
            VALUES ('20221229110455', '2023-12-23 09:27:06');
            COMMIT;
        "#;

            info!("Creating migration table...");
            conn.batch_execute(MIGRATION_TABLE_SQL).map_err(|e| {
                ApplicationError::Other(format!("Failed to create migrations table: {}", e))
            })?;
        }

        let pending = conn.pending_migrations(MIGRATIONS).map_err(|e| {
            ApplicationError::Other(format!("Failed to get pending migrations: {}", e))
        })?;

        pending.iter().for_each(|m| {
            debug!("Pending Migration: {}", m.name());
        });

        // Fix: Handle the returned migrations vector by discarding it
        let _applied_migrations = conn.run_pending_migrations(MIGRATIONS).map_err(|e| {
            ApplicationError::Other(format!("Failed to run pending migrations: {}", e))
        })?;

        // Return success
        Ok(())
    }

    fn check_if_migrations_table_exists(
        &self,
        conn: &mut diesel::SqliteConnection,
    ) -> Result<bool, diesel::result::Error> {
        use diesel::prelude::*;
        use diesel::sql_query;
        use diesel::sql_types::Integer;

        #[derive(QueryableByName, Debug)]
        struct ExistenceCheck {
            #[diesel(sql_type = Integer)]
            diesel_exists: i32,
        }

        let query = "
            SELECT 1 as diesel_exists FROM sqlite_master WHERE type='table' AND name='__diesel_schema_migrations';
        ";

        let result: Vec<ExistenceCheck> = sql_query(query).load(conn)?;

        Ok(!result.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_check_migrations_table_exists() {
        // Create a temporary file
        let tmp_file = NamedTempFile::new().expect("Failed to create temp file");
        let db_path = tmp_file.path().to_str().unwrap().to_string();

        // Create service
        let service = MigrationService::new(db_path);

        // Create repository and connection to initialize the database
        use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
        let repo = SqliteBookmarkRepository::from_url(&service.repository_url)
            .expect("Failed to create repository");

        let mut conn = repo.get_connection().expect("Failed to get connection");

        // Check before creating the table
        let exists_before = service.check_if_migrations_table_exists(&mut conn).unwrap();
        assert!(!exists_before, "Migrations table should not exist yet");

        // Create the migrations table
        conn.batch_execute(
            "CREATE TABLE __diesel_schema_migrations (
                version VARCHAR(50) PRIMARY KEY NOT NULL,
                run_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("Failed to create migrations table");

        // Check after creating the table
        let exists_after = service.check_if_migrations_table_exists(&mut conn).unwrap();
        assert!(exists_after, "Migrations table should exist now");
    }
}
