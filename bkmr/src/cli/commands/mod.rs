// src/cli/commands/mod.rs

use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use termcolor::StandardStream;
use tracing::{info, instrument};

// Import submodules
pub mod bookmark_commands;
pub mod tag_commands;

// Function to execute the appropriate command based on CLI arguments
pub fn execute_command(stderr: StandardStream, cli: Cli) -> CliResult<()> {
    match cli.command {
        Some(Commands::Search { .. }) => bookmark_commands::search(stderr, cli),
        Some(Commands::SemSearch { .. }) => bookmark_commands::semantic_search(stderr, cli),
        Some(Commands::Open { .. }) => bookmark_commands::open(cli),
        Some(Commands::Add { .. }) => bookmark_commands::add(cli),
        Some(Commands::Delete { .. }) => bookmark_commands::delete(cli),
        Some(Commands::Update { .. }) => bookmark_commands::update(cli),
        Some(Commands::Edit { .. }) => bookmark_commands::edit(cli),
        Some(Commands::Show { .. }) => bookmark_commands::show(cli),
        Some(Commands::Tags { .. }) => tag_commands::show_tags(cli),
        Some(Commands::CreateDb { .. }) => bookmark_commands::create_db(cli),
        Some(Commands::Surprise { .. }) => bookmark_commands::surprise(cli),
        Some(Commands::Backfill { .. }) => bookmark_commands::backfill(cli),
        Some(Commands::LoadTexts { .. }) => bookmark_commands::load_texts(cli),
        Some(Commands::Xxx { ids, tags }) => {
            eprintln!("ids: {:?}, tags: {:?}", ids, tags);
            Ok(())
        }
        None => Ok(()),
    }
}

#[instrument(level = "debug")]
pub fn enable_embeddings_if_required() -> CliResult<()> {
    use crate::application::services::migration_service::MigrationService;
    use crate::environment::CONFIG;
    use crate::util::helper::confirm;
    use crossterm::style::Stylize;

    // Create the migration service from the application layer
    let service = MigrationService::new(CONFIG.db_url.clone());

    // Check if embedding column exists
    let embedding_exists = service
        .check_embedding_column_exists()
        .map_err(CliError::Application)?;

    if embedding_exists {
        info!("Embedding column already exists, no action required.");
        return Ok(());
    }

    eprintln!("New 'bkmr' version requires an extension of the database schema.");
    eprintln!("Two new columns will be added to the 'bookmarks' table:");

    if !confirm("Please backup up your DB before continue! Do you want to continue?") {
        return Err(CliError::OperationAborted);
    }

    // Run migrations
    service.run_migrations().map_err(CliError::Application)?;

    eprintln!("{}", "Database schema has been extended.".blue());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::services::bookmark_application_service::BookmarkApplicationService;
    use crate::cli::args::Cli;
    use crate::cli::error::CliResult;
    use crate::context::{Context, CTX};
    use crate::infrastructure::embeddings::DummyEmbedding;
    use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
    use clap::Parser;
    use std::fs;
    use std::path::Path;
    use std::sync::RwLock;
    use tempfile::tempdir;
    use termcolor::ColorChoice;
    use termcolor::StandardStream;

    // Helper function to set up a test environment
    fn setup_test_environment(db_name: &str) -> CliResult<(String, StandardStream)> {
        // Create temporary directory
        let temp_dir = tempdir().expect("Failed to create temp directory");

        // Create a test database path
        let db_path = temp_dir.path().join(db_name).to_string_lossy().to_string();

        // Set environment variable to use this database
        std::env::set_var("BKMR_DB_URL", &db_path);

        // Initialize context with dummy embedding
        let context = Context::new(Box::new(DummyEmbedding));
        if CTX.set(RwLock::from(context)).is_err() {
            return Err(crate::cli::error::CliError::Other(
                "Failed to initialize context".to_string(),
            ));
        }

        // Create stderr for command output
        let stderr = StandardStream::stderr(ColorChoice::Never);

        Ok((db_path, stderr))
    }

    // Helper to create a bookmark application service
    fn create_service(
        db_path: &str,
    ) -> Result<BookmarkApplicationService<SqliteBookmarkRepository>, crate::cli::error::CliError>
    {
        let repo = SqliteBookmarkRepository::from_url(db_path)
            .map_err(|e| crate::cli::error::CliError::RepositoryError(e.to_string()))?;

        Ok(BookmarkApplicationService::new(repo))
    }

    #[test]
    fn test_create_db_command() -> CliResult<()> {
        // Set up test environment
        let (db_path, stderr) = setup_test_environment("create_test.db")?;

        // Parse command line arguments
        let cli = Cli::parse_from(["bkmr", "create-db", &db_path]);

        // Execute command
        let result = execute_command(stderr, cli);

        // Check that command executed successfully
        assert!(result.is_ok(), "Command should execute successfully");

        // Check that database file was created
        assert!(Path::new(&db_path).exists(), "Database file should exist");

        // Clean up
        if Path::new(&db_path).exists() {
            fs::remove_file(&db_path).ok();
        }

        Ok(())
    }

    #[test]
    fn test_add_and_show_bookmark() -> CliResult<()> {
        // Set up test environment
        let (db_path, _stderr) = setup_test_environment("add_show_test.db")?;

        // Create database first
        let create_cli = Cli::parse_from(["bkmr", "create-db", &db_path]);
        // Create a new stderr for each command execution
        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, create_cli)?;

        // Add a bookmark
        let add_cli = Cli::parse_from([
            "bkmr",
            "add",
            "https://example.com",
            "--title",
            "Example Website",
            "--description",
            "An example website for testing",
            "--tags",
            "test,example",
            "--no-web",
        ]);

        // Create a new stderr
        let add_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(add_stderr, add_cli)?;

        // Use the application service to verify bookmark was added
        let service = create_service(&db_path)?;
        let bookmarks = service.get_all_bookmarks()?;

        // Should have one bookmark
        assert_eq!(bookmarks.len(), 1, "Should have 1 bookmark");

        // Check bookmark data
        let bookmark = &bookmarks[0];
        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(bookmark.title, "Example Website");
        assert_eq!(bookmark.description, "An example website for testing");

        // Check tags
        assert!(bookmark.tags.contains(&"test".to_string()));
        assert!(bookmark.tags.contains(&"example".to_string()));

        // Test show command
        let show_cli = Cli::parse_from(["bkmr", "show", &bookmark.id.unwrap().to_string()]);

        // Create a new stderr
        let show_stderr = StandardStream::stderr(ColorChoice::Never);
        // Execute show command (just checking that it doesn't error)
        let show_result = execute_command(show_stderr, show_cli);
        assert!(
            show_result.is_ok(),
            "Show command should execute successfully"
        );

        // Clean up
        if Path::new(&db_path).exists() {
            fs::remove_file(&db_path).ok();
        }

        Ok(())
    }

    #[test]
    fn test_delete_bookmark() -> CliResult<()> {
        // Set up test environment
        let (db_path, stderr) = setup_test_environment("delete_test.db")?;

        // Create database first
        let create_cli = Cli::parse_from(["bkmr", "create-db", &db_path]);
        // Create a new stderr for each command execution
        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, create_cli)?;

        // Add a bookmark
        let add_cli = Cli::parse_from([
            "bkmr",
            "add",
            "https://example.com",
            "--tags",
            "test",
            "--no-web",
        ]);

        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, add_cli)?;

        // Use application service to get bookmarks
        let service = create_service(&db_path)?;
        let bookmarks = service.get_all_bookmarks()?;
        assert!(!bookmarks.is_empty(), "Should have at least one bookmark");

        let id = bookmarks[0].id.unwrap();

        // Use service to delete (non-interactive test)
        service.delete_bookmark(id)?;

        // Verify bookmark is deleted
        let after_delete = service.get_bookmark(id)?;
        assert!(after_delete.is_none(), "Bookmark should be deleted");

        // Clean up
        if Path::new(&db_path).exists() {
            fs::remove_file(&db_path).ok();
        }

        Ok(())
    }

    #[test]
    fn test_update_bookmark() -> CliResult<()> {
        // Set up test environment
        let (db_path, stderr) = setup_test_environment("update_test.db")?;

        // Create database first
        let create_cli = Cli::parse_from(["bkmr", "create-db", &db_path]);
        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, create_cli)?;

        // Add a bookmark
        let add_cli = Cli::parse_from([
            "bkmr",
            "add",
            "https://example.com",
            "--tags",
            "initial",
            "--no-web",
        ]);

        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, add_cli)?;

        // Use application service to get bookmarks
        let service = create_service(&db_path)?;
        let bookmarks = service.get_all_bookmarks()?;
        assert!(!bookmarks.is_empty(), "Should have at least one bookmark");

        let id = bookmarks[0].id.unwrap();

        // Update the bookmark
        let update_cli =
            Cli::parse_from(["bkmr", "update", &id.to_string(), "--tags", "updated,new"]);

        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, update_cli)?;

        // Verify tags were updated
        let updated = service.get_bookmark(id)?;
        assert!(updated.is_some(), "Bookmark should exist");

        let updated = updated.unwrap();

        // Should have both original and new tags
        assert!(
            updated.tags.contains(&"initial".to_string()),
            "Should keep initial tag"
        );
        assert!(
            updated.tags.contains(&"updated".to_string()),
            "Should add updated tag"
        );
        assert!(
            updated.tags.contains(&"new".to_string()),
            "Should add new tag"
        );

        // Update with force flag to replace tags
        let force_update_cli = Cli::parse_from([
            "bkmr",
            "update",
            &id.to_string(),
            "--tags",
            "forced",
            "--force",
        ]);

        let create_stderr = StandardStream::stderr(ColorChoice::Never);
        execute_command(create_stderr, force_update_cli)?;

        // Verify tags were replaced
        let force_updated = service.get_bookmark(id)?;
        assert!(force_updated.is_some(), "Bookmark should exist");

        let force_updated = force_updated.unwrap();

        // Should only have the new forced tag
        assert_eq!(force_updated.tags.len(), 1, "Should have exactly 1 tag");
        assert!(
            force_updated.tags.contains(&"forced".to_string()),
            "Should only have forced tag"
        );

        // Clean up
        if Path::new(&db_path).exists() {
            fs::remove_file(&db_path).ok();
        }

        Ok(())
    }
}
