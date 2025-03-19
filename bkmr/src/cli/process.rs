// src/cli/process.rs

use indoc::formatdoc;
use regex::Regex;
use std::fs::{self};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use tempfile::NamedTempFile;
use tracing::debug;

use crate::application::dto::{BookmarkResponse, BookmarkUpdateRequest};
use crate::application::services::bookmark_application_service::BookmarkApplicationService;
use crate::cli::create_bookmark_service;
use crate::cli::display::{show_bookmarks, DisplayBookmark, ALL_FIELDS, DEFAULT_FIELDS};
use crate::cli::error::{CliError, CliResult};
use crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository;

/// Process a list of bookmarks interactively
pub fn process(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    if bookmarks.is_empty() {
        return Ok(());
    }

    let help_text = r#"
        <n1> <n2>:      opens selection in browser
        p <n1> <n2>:    print id-list of selection
        p:              print all ids
        d <n1> <n2>:    delete selection
        e <n1> <n2>:    edit selection
        t <n1> <n2>:    touch selection (update timestamp)
        q | ENTER:      quit
        h:              help
    "#;

    loop {
        eprint!("> ");
        io::stdout().flush().map_err(CliError::Io)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(CliError::Io)?;

        let tokens = parse_input(&input);
        if tokens.is_empty() {
            break;
        }

        let regex = Regex::new(r"^\d+").unwrap();
        match tokens[0].as_str() {
            "p" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    print_bookmark_ids(indices, bookmarks)?;
                } else if tokens.len() == 1 {
                    // Print all ids
                    print_all_bookmark_ids(bookmarks)?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "d" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    delete_bookmarks_by_indices(indices, bookmarks)?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "e" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    edit_bookmarks_by_indices(indices, bookmarks)?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "t" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    touch_bookmarks_by_indices(indices, bookmarks)?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "h" => println!("{}", help_text),
            "q" => break,
            s if regex.is_match(s) => {
                if let Some(indices) = ensure_int_vector(&tokens) {
                    open_bookmarks_by_indices(indices, bookmarks)?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            _ => {
                println!("Invalid Input");
                println!("{}", help_text);
            }
        }
    }

    Ok(())
}

/// Parse input string into tokens
fn parse_input(input: &str) -> Vec<String> {
    input
        .trim()
        .replace(',', " ")
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Convert vector of strings to vector of integers if all strings are valid integers
fn ensure_int_vector(vec: &[String]) -> Option<Vec<i32>> {
    vec.iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

/// Get bookmark by index in the displayed list
fn get_bookmark_by_index(index: i32, bookmarks: &[BookmarkResponse]) -> Option<&BookmarkResponse> {
    if index < 1 || index as usize > bookmarks.len() {
        return None;
    }
    Some(&bookmarks[index as usize - 1])
}

/// Open a bookmark
pub(crate) fn open_bookmark(bookmark: &BookmarkResponse) -> CliResult<()> {
    // First record access/touch bookmark
    if let Some(id) = bookmark.id {
        let service = create_bookmark_service()?;
        service
            .record_bookmark_access(id)
            .map_err(CliError::Application)?;
    }

    let url = &bookmark.url;

    // Handle special protocol URLs (shell::)
    if url.starts_with("shell::") {
        let command = &url[7..];
        debug!("Executing shell command: {}", command);

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

    // Handle files and URLs
    let path = Path::new(url);
    if path.exists() {
        // For markdown files, open with editor
        if path.extension().is_some_and(|ext| ext == "md") {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            debug!("Opening markdown file with editor: {}", editor);

            Command::new(&editor)
                .arg(url)
                .status()
                .map_err(|e| CliError::CommandFailed(format!("Failed to open editor: {}", e)))?;

            return Ok(());
        }
    }

    // For everything else, use the system's default handler
    open::that(url).map_err(|e| CliError::CommandFailed(format!("Failed to open URL: {}", e)))?;

    Ok(())
}

/// Open bookmarks by their indices in the displayed list
fn open_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    debug!("Opening bookmarks at indices: {:?}", indices);

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => open_bookmark(bookmark)?,
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

/// Touch (update timestamp) of bookmarks by indices
fn touch_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    debug!("Touching bookmarks at indices: {:?}", indices);

    let service = create_bookmark_service()?;

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                if let Some(id) = bookmark.id {
                    service
                        .record_bookmark_access(id)
                        .map_err(CliError::Application)?;

                    // Display the updated bookmark
                    if let Ok(Some(updated)) = service.get_bookmark(id) {
                        show_bookmarks(&[DisplayBookmark::from_dto(&updated)], ALL_FIELDS);
                    }
                }
            }
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

/// Print IDs of bookmarks by indices
fn print_bookmark_ids(indices: Vec<i32>, bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    let mut ids = Vec::new();

    for index in indices {
        if let Some(bookmark) = get_bookmark_by_index(index, bookmarks) {
            if let Some(id) = bookmark.id {
                ids.push(id);
            }
        } else {
            eprintln!("Index {} out of range", index);
        }
    }

    ids.sort();
    println!(
        "{}",
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    Ok(())
}

/// Print IDs of all bookmarks
fn print_all_bookmark_ids(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    let mut ids: Vec<_> = bookmarks.iter().filter_map(|b| b.id).collect();

    ids.sort();
    println!(
        "{}",
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );

    Ok(())
}

/// Edit bookmarks by their indices
fn edit_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    // Get IDs from indices
    let mut bookmark_ids = Vec::new();
    for index in indices {
        if let Some(bookmark) = get_bookmark_by_index(index, bookmarks) {
            if let Some(id) = bookmark.id {
                bookmark_ids.push(id);
            }
        } else {
            eprintln!("Index {} out of range", index);
        }
    }

    // Call the edit function with actual IDs
    edit_bookmarks(bookmark_ids)
}

/// Edit bookmarks by IDs
pub fn edit_bookmarks(ids: Vec<i32>) -> CliResult<()> {
    let service = create_bookmark_service()?;
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
        let template = formatdoc! {r###"
            # Bookmark ID: {}
            URL: {}
            Title: {}
            Description: {}
            Tags: {}

            "###,
            bookmark.id.unwrap_or(0),
            bookmark.url,
            bookmark.title,
            bookmark.description,
            bookmark.tags.join(","),
        };

        temp_file
            .write_all(template.as_bytes())
            .map_err(CliError::Io)?;
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

/// Update a bookmark with new information
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

/// Delete bookmarks by their indices in the displayed list
fn delete_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    // Get IDs from indices
    let mut bookmark_ids = Vec::new();
    for index in indices {
        if let Some(bookmark) = get_bookmark_by_index(index, bookmarks) {
            if let Some(id) = bookmark.id {
                bookmark_ids.push(id);
            }
        } else {
            eprintln!("Index {} out of range", index);
        }
    }

    // Call the delete function with actual IDs
    delete_bookmarks(bookmark_ids)
}

/// Delete bookmarks by their IDs
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

    // Sort IDs in reverse to handle database compaction correctly
    // (same as in original implementation)
    let mut sorted_ids = ids.clone();
    sorted_ids.sort_by(|a, b| b.cmp(a)); // Reverse sort

    // Delete bookmarks through the service
    let mut deleted_count = 0;
    for id in sorted_ids {
        if let Ok(true) = service.delete_bookmark(id) {
            deleted_count += 1;
        }
    }

    println!("Deleted {} bookmarks", deleted_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input() {
        let input = "  1 2,  3  ";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["1", "2", "3"]);

        let input = "p 1,2,3";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["p", "1", "2", "3"]);
    }

    #[test]
    fn test_ensure_int_vector() {
        let tokens = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let result = ensure_int_vector(&tokens);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);

        let tokens = vec!["1".to_string(), "abc".to_string()];
        let result = ensure_int_vector(&tokens);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_bookmark_by_index() {
        let bookmarks = vec![
            BookmarkResponse {
                id: Some(10),
                url: "https://example.com".to_string(),
                title: "Example".to_string(),
                description: "An example site".to_string(),
                tags: vec!["example".to_string()],
                access_count: 0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            BookmarkResponse {
                id: Some(20),
                url: "https://test.com".to_string(),
                title: "Test".to_string(),
                description: "A test site".to_string(),
                tags: vec!["test".to_string()],
                access_count: 0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        ];

        // Valid index
        let bookmark = get_bookmark_by_index(1, &bookmarks);
        assert!(bookmark.is_some());
        assert_eq!(bookmark.unwrap().id, Some(10));

        // Out of range
        let bookmark = get_bookmark_by_index(3, &bookmarks);
        assert!(bookmark.is_none());

        // Negative index (invalid)
        let bookmark = get_bookmark_by_index(-1, &bookmarks);
        assert!(bookmark.is_none());
    }
}
