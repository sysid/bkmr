// src/cli/commands/tag_commands.rs
use crate::application::dto::tag_dto::{TagMergeRequest, TagOperationRequest, TagRenameRequest};
use crate::application::services::tag_application_service::TagApplicationService;
use crate::cli::args::{Cli, Commands};
use crate::cli::error::{CliError, CliResult};
use crate::environment::CONFIG;
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;
use crate::util::helper::confirm;

use tracing::instrument;

// Create a service factory function to reduce boilerplate
fn create_tag_service() -> CliResult<TagApplicationService<SqliteBookmarkRepository>> {
    SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| CliError::RepositoryError(format!("Failed to create repository: {}", e)))
        .map(TagApplicationService::new)
}

#[instrument(skip(cli))]
pub fn show_tags(cli: Cli) -> CliResult<()> {
    if let Commands::Tags { tag } = cli.command.unwrap() {
        let service = create_tag_service()?;

        // Get tag data
        let tags = match tag {
            Some(ref tag_name) => service.get_related_tags(tag_name)?,
            None => service.get_all_tags()?,
        };

        // Display tags
        for tag_info in tags {
            println!("{}: {}", tag_info.count, tag_info.name);
        }
    }
    Ok(())
}

#[instrument]
pub fn merge_tags(name1: &str, name2: &str) -> CliResult<()> {
    // Confirm operation
    if !confirm(&format!("Merge tag '{}' into '{}'?", name1, name2)) {
        return Err(CliError::OperationAborted);
    }

    let service = create_tag_service()?;

    let request = TagMergeRequest {
        source_tag: name1.to_string(),
        target_tag: name2.to_string(),
    };

    let count = service.merge_tags(request)?;
    println!(
        "Updated {} bookmarks, merged '{}' into '{}'",
        count, name1, name2
    );

    Ok(())
}

#[instrument]
pub fn rename_tag(old_name: &str, new_name: &str) -> CliResult<()> {
    // Confirm operation
    if !confirm(&format!("Rename tag '{}' to '{}'?", old_name, new_name)) {
        return Err(CliError::OperationAborted);
    }

    let service = create_tag_service()?;

    let request = TagRenameRequest {
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
    };

    let count = service.rename_tag(request)?;
    println!(
        "Updated {} bookmarks, renamed '{}' to '{}'",
        count, old_name, new_name
    );

    Ok(())
}

#[instrument]
pub fn add_tags_to_bookmarks(
    bookmark_ids: Vec<i32>,
    tag_names: Vec<String>,
    replace: bool,
) -> CliResult<()> {
    let service = create_tag_service()?;

    let request = TagOperationRequest {
        bookmark_ids,
        tags: tag_names,
        replace_existing: Some(replace),
    };

    let count = service.add_tags_to_bookmarks(request)?;
    println!("Added tags to {} bookmarks", count);

    Ok(())
}

#[instrument]
pub fn remove_tags_from_bookmarks(bookmark_ids: Vec<i32>, tag_names: Vec<String>) -> CliResult<()> {
    let service = create_tag_service()?;

    let request = TagOperationRequest {
        bookmark_ids,
        tags: tag_names,
        replace_existing: None,
    };

    let count = service.remove_tags_from_bookmarks(request)?;
    println!("Removed tags from {} bookmarks", count);

    Ok(())
}

#[instrument]
pub fn get_tag_suggestions(partial: &str) -> CliResult<Vec<String>> {
    let service = create_tag_service()?;

    let suggestions = service.get_tag_suggestions(partial)?;
    Ok(suggestions
        .suggestions
        .into_iter()
        .map(|t| t.name)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::{fixture, rstest};
    use std::fs;

    #[fixture]
    fn temp_dir() -> Utf8PathBuf {
        let tempdir = tempdir().unwrap();
        let options = dir::CopyOptions::new().overwrite(true);
        copy_items(
            &["tests/resources/bkmr.v1.db", "tests/resources/bkmr.v2.db"],
            "../db",
            &options,
        )
        .expect("Failed to copy test project directory");

        fs::rename("../db/bkmr.v2.db", "../db/bkmr.db").expect("Failed to rename database");

        tempdir.into_path()
    }

    #[ignore = "requires database setup and user confirmation"]
    #[rstest]
    fn test_get_tag_suggestions(temp_dir: Utf8PathBuf) -> CliResult<()> {
        // Arrange: Set up environment
        let partial = "ru"; // Looking for tags starting with "ru"

        // Act: Get suggestions
        let suggestions = get_tag_suggestions(partial)?;

        // Assert: Should find some results
        assert!(
            !suggestions.is_empty(),
            "Expected at least one tag suggestion"
        );

        // All suggestions should contain the partial string
        for suggestion in &suggestions {
            assert!(
                suggestion.to_lowercase().contains(partial),
                "Suggestion '{}' doesn't contain the search term '{}'",
                suggestion,
                partial
            );
        }

        Ok(())
    }
}
