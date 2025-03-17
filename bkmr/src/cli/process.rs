// src/cli/process.rs

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;
use tracing::{error, info};

use crate::application::dto::{BookmarkResponse, BookmarkUpdateRequest};
use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::cli::create_bookmark_service;
use crate::cli::display::{show_bookmarks, DisplayBookmark, DEFAULT_FIELDS};
use crate::cli::error::{CliError, CliResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::environment::CONFIG;
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;

pub fn process(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    if bookmarks.is_empty() {
        return Ok(());
    }

    let repo = SqliteBookmarkRepository::from_url(&CONFIG.db_url)
        .map_err(|e| CliError::RepositoryError(format!("Failed to create repository: {}", e)))?;

    let service = BookmarkApplicationService::new(repo);

    loop {
        let mut input = String::new();
        print!("Enter bookmark ID to open (or q to quit): ");
        io::stdout().flush().map_err(CliError::Io)?;
        io::stdin().read_line(&mut input).map_err(CliError::Io)?;

        let input = input.trim();
        if input.eq_ignore_ascii_case("q") {
            break;
        }

        if let Ok(id) = input.parse::<i32>() {
            if let Ok(Some(bookmark)) = service.get_bookmark(id) {
                open_bookmark(&bookmark)?;
            } else {
                println!("Bookmark with ID {} not found", id);
            }
        } else {
            println!("Invalid input. Please enter a numeric ID or 'q' to quit.");
        }
    }

    Ok(())
}

pub fn open_bookmark(bookmark: &BookmarkResponse) -> CliResult<()> {
    let url = &bookmark.url;

    // Handle special protocol URLs like shell::command
    if url.starts_with("shell::") {
        let command = &url[7..];
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| {
                CliError::CommandFailed(format!("Failed to execute shell command: {}", e))
            })?;

        io::stdout()
            .write_all(&output.stdout)
            .map_err(CliError::Io)?;
        io::stderr()
            .write_all(&output.stderr)
            .map_err(CliError::Io)?;

        return Ok(());
    }

    // Open URLs and files
    open::that(url).map_err(|e| CliError::CommandFailed(format!("Failed to open URL: {}", e)))?;

    Ok(())
}

pub fn edit_bookmarks(ids: Vec<i32>) -> CliResult<()> {
    let service = crate::cli::create_bookmark_service()?;
    let mut bookmarks_to_edit = Vec::new();

    // Fetch bookmarks using service
    for id in &ids {
        if let Ok(Some(bookmark)) = service.get_bookmark(*id) {
            bookmarks_to_edit.push(bookmark);
        } else {
            println!("Bookmark with ID {} not found", id);
        }
    }

    if bookmarks_to_edit.is_empty() {
        println!("No bookmarks found to edit");
        return Ok(());
    }

    // Display bookmarks before editing
    let display_bookmarks: Vec<_> = bookmarks_to_edit
        .iter()
        .map(DisplayBookmark::from_dto)
        .collect();

    show_bookmarks(&display_bookmarks, DEFAULT_FIELDS);

    // Create temporary file with bookmark data
    let mut temp_file = NamedTempFile::new().map_err(CliError::Io)?;

    for bookmark in &bookmarks_to_edit {
        writeln!(temp_file, "# Bookmark ID: {}", bookmark.id.unwrap_or(0)).map_err(CliError::Io)?;
        writeln!(temp_file, "URL: {}", bookmark.url).map_err(CliError::Io)?;
        writeln!(temp_file, "Title: {}", bookmark.title).map_err(CliError::Io)?;
        writeln!(temp_file, "Description: {}", bookmark.description).map_err(CliError::Io)?;

        // Format tags as comma-separated without the surrounding commas
        let tags_str = bookmark.tags.join(",");

        writeln!(temp_file, "Tags: {}", tags_str).map_err(CliError::Io)?;
        writeln!(temp_file).map_err(CliError::Io)?;
    }

    temp_file.flush().map_err(CliError::Io)?;

    // Get editor from environment or use a default
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // Open editor
    let status = Command::new(&editor)
        .arg(temp_file.path())
        .status()
        .map_err(|e| CliError::CommandFailed(format!("Failed to open editor: {}", e)))?;

    if !status.success() {
        return Err(CliError::CommandFailed(
            "Editor exited with error".to_string(),
        ));
    }

    // Read the edited file
    let content = fs::read_to_string(temp_file.path()).map_err(CliError::Io)?;

    // Parse the edited content and update bookmarks
    let mut current_id = None;
    let mut current_url = String::new();
    let mut current_title = String::new();
    let mut current_description = String::new();
    let mut current_tags = String::new();
    let mut updated_count = 0;

    for line in content.lines() {
        if line.starts_with("# Bookmark ID:") {
            // Process previous bookmark if we have one
            if let Some(id) = current_id {
                if update_bookmark(
                    &service,
                    id,
                    &current_url,
                    &current_title,
                    &current_description,
                    &current_tags,
                )? {
                    updated_count += 1;
                }
            }

            // Parse new bookmark ID
            current_id = line
                .trim_start_matches("# Bookmark ID:")
                .trim()
                .parse::<i32>()
                .ok();
            current_url = String::new();
            current_title = String::new();
            current_description = String::new();
            current_tags = String::new();
        } else if line.starts_with("URL:") {
            current_url = line.trim_start_matches("URL:").trim().to_string();
        } else if line.starts_with("Title:") {
            current_title = line.trim_start_matches("Title:").trim().to_string();
        } else if line.starts_with("Description:") {
            current_description = line.trim_start_matches("Description:").trim().to_string();
        } else if line.starts_with("Tags:") {
            current_tags = line.trim_start_matches("Tags:").trim().to_string();
        }
    }

    // Process the last bookmark
    if let Some(id) = current_id {
        if update_bookmark(
            &service,
            id,
            &current_url,
            &current_title,
            &current_description,
            &current_tags,
        )? {
            updated_count += 1;
        }
    }

    println!("Updated {} bookmarks", updated_count);
    Ok(())
}

fn update_bookmark(
    service: &BookmarkApplicationService<SqliteBookmarkRepository>,
    id: i32,
    url: &str,
    title: &str,
    description: &str,
    tags_str: &str,
) -> CliResult<bool> {
    // Get the current bookmark
    let bookmark = service
        .get_bookmark(id)?
        .ok_or_else(|| CliError::CommandFailed(format!("Bookmark with ID {} not found", id)))?;

    // Check if anything changed
    if bookmark.url == url
        && bookmark.title == title
        && bookmark.description == description
        && bookmark.tags.join(",") == tags_str
    {
        return Ok(false);
    }

    // Create updated bookmark request
    let request = BookmarkUpdateRequest {
        id,
        title: Some(title.to_string()),
        description: Some(description.to_string()),
        tags: Some(
            tags_str
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        ),
    };

    // Update the bookmark
    service.update_bookmark(request)?;

    Ok(true)
}

pub fn delete_bookmarks(ids: Vec<i32>) -> CliResult<()> {
    let service = create_bookmark_service()?;

    // Display bookmarks to be deleted
    let mut bookmarks_to_display = Vec::new();
    for id in &ids {
        if let Ok(Some(bookmark)) = service.get_bookmark(*id) {
            bookmarks_to_display.push(DisplayBookmark::from_dto(&bookmark));
        }
    }

    if bookmarks_to_display.is_empty() {
        println!("No bookmarks found to delete");
        return Ok(());
    }

    show_bookmarks(&bookmarks_to_display, DEFAULT_FIELDS);

    // Confirm deletion
    print!("Delete these bookmarks? (y/N): ");
    io::stdout().flush().map_err(CliError::Io)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(CliError::Io)?;

    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        return Err(CliError::OperationAborted);
    }

    // Delete bookmarks through the service
    let mut deleted_count = 0;
    for id in ids {
        if let Ok(true) = service.delete_bookmark(id) {
            deleted_count += 1;
        }
    }

    println!("Deleted {} bookmarks", deleted_count);
    Ok(())
}
