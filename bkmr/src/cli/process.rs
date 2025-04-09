// src/cli/process.rs

use regex::Regex;
use std::io::{self, Write};
use tracing::{debug, instrument};

use crate::application::services::factory::{
    create_bookmark_service, create_clipboard_service, create_interpolation_service,
    create_template_service, create_action_service,
};
use crate::cli::display::{show_bookmarks, DisplayBookmark, ALL_FIELDS, DEFAULT_FIELDS};
use crate::cli::error::{CliError, CliResult};
use crate::domain::bookmark::Bookmark;
use crate::util::helper::{confirm, ensure_int_vector};

/// Process a list of bookmarks interactively
#[instrument(skip_all, level = "debug")]
pub fn process(bookmarks: &[Bookmark]) -> CliResult<()> {
    if bookmarks.is_empty() {
        return Ok(());
    }

    let help_text = r#"
       <n1> <n2>:      performs default action on selection (open URI, copy snippet, etc.)
       p <n1> <n2>:    print id-list of selection
       p:              print all ids
       d <n1> <n2>:    delete selection
       e <n1> <n2>:    edit selection
       t <n1> <n2>:    touch selection (update timestamp)
       q | ENTER:      quit
       h:              help
   "#;

    let regex = Regex::new(r"^\d+").unwrap();
    loop {
        eprint!("> ");
        io::stdout().flush().map_err(CliError::Io)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(CliError::Io)?;

        let tokens = parse_input(&input);
        if tokens.is_empty() {
            break;
        }

        match tokens[0].as_str() {
            "p" => {
                if tokens.len() == 1 {
                    // Just "p" command, print all ids
                    print_all_bookmark_ids(bookmarks)?;
                } else if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    // "p" with indices
                    print_bookmark_ids(indices, bookmarks)?;
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
                    // Instead of just opening, perform the default action
                    execute_default_actions_by_indices(indices, bookmarks)?;
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
#[instrument(level = "trace")]
fn parse_input(input: &str) -> Vec<String> {
    input
        .trim()
        .replace(',', " ")
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Get bookmark by index in the displayed list
#[instrument(skip(bookmarks), level = "trace")]
fn get_bookmark_by_index(index: i32, bookmarks: &[Bookmark]) -> Option<&Bookmark> {
    if index < 1 || index as usize > bookmarks.len() {
        return None;
    }
    Some(&bookmarks[index as usize - 1])
}

/// Executes the default action for a bookmark
#[instrument(level = "debug")]
pub fn execute_bookmark_default_action(bookmark: &Bookmark) -> CliResult<()> {
    let action_service = create_action_service();

    // Get action description for logging
    let action_description = action_service.get_default_action_description(bookmark);
    debug!("Executing default action: {} for bookmark: {}", action_description, bookmark.title);

    // Execute the default action
    action_service.execute_default_action(bookmark)?;

    Ok(())
}

/// Executes default actions for bookmarks by their indices
#[instrument(skip(bookmarks), level = "debug")]
fn execute_default_actions_by_indices(indices: Vec<i32>, bookmarks: &[Bookmark]) -> CliResult<()> {
    debug!("Executing default actions for bookmarks at indices: {:?}", indices);

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                // Get the action service and determine the action type
                let action_service = create_action_service();
                let action_type = action_service.get_default_action_description(bookmark);

                // Show what we're doing
                println!(
                    "Executing '{}' for bookmark: {} (ID: {})",
                    action_type,
                    bookmark.title,
                    bookmark.id.unwrap_or(0)
                );

                // Execute the action
                execute_bookmark_default_action(bookmark)?
            }
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

// For backward compatibility
#[instrument(level = "debug")]
pub fn open_bookmark(bookmark: &Bookmark) -> CliResult<()> {
    // This now delegates to the default action system
    execute_bookmark_default_action(bookmark)
}

/// Touch (update timestamp) of bookmarks by indices
#[instrument(skip(bookmarks), level = "debug")]
fn touch_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[Bookmark]) -> CliResult<()> {
    debug!("Touching bookmarks at indices: {:?}", indices);
    let service = create_bookmark_service();

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                if let Some(id) = bookmark.id {
                    service
                        .record_bookmark_access(id)
                        .map_err(CliError::Application)?;

                    // Display the updated bookmark
                    if let Ok(Some(updated)) = service.get_bookmark(id) {
                        show_bookmarks(&[DisplayBookmark::from_domain(&updated)], ALL_FIELDS);
                    }
                }
            }
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

/// Print IDs of bookmarks by indices
#[instrument(skip(bookmarks), level = "debug")]
fn print_bookmark_ids(indices: Vec<i32>, bookmarks: &[Bookmark]) -> CliResult<()> {
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

    if ids.is_empty() {
        println!("No bookmark IDs found for the specified indices");
        io::stdout().flush().map_err(CliError::Io)?;
        return Ok(());
    }

    ids.sort();
    println!(
        "{}",
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    io::stdout().flush().map_err(CliError::Io)?; // todo: check if necessary

    Ok(())
}

/// Print IDs of all bookmarks
#[instrument(skip(bookmarks), level = "debug")]
fn print_all_bookmark_ids(bookmarks: &[Bookmark]) -> CliResult<()> {
    let mut ids: Vec<_> = bookmarks.iter().filter_map(|b| b.id).collect();

    if ids.is_empty() {
        println!("No bookmark IDs found");
        io::stdout().flush().map_err(CliError::Io)?; // todo: check if this is needed
        return Ok(());
    }

    // Print the count for verification
    println!("Found {} bookmark IDs", ids.len());

    // Sort and print the IDs
    ids.sort();
    println!(
        "{}",
        ids.iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    io::stdout().flush().map_err(CliError::Io)?; // todo: check if this is needed

    Ok(())
}

/// Edit bookmarks by their indices
#[instrument(skip(bookmarks), level = "debug")]
fn edit_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[Bookmark]) -> CliResult<()> {
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
#[instrument(level = "debug")]
pub fn edit_bookmarks(ids: Vec<i32>) -> CliResult<()> {
    let bookmark_service = create_bookmark_service();
    let template_service = create_template_service();
    let mut bookmarks_to_edit = Vec::new();
    let mut updated_count = 0;

    // Fetch bookmarks
    for id in &ids {
        if let Ok(Some(bookmark)) = bookmark_service.get_bookmark(*id) {
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
        .map(DisplayBookmark::from_domain)
        .collect();

    show_bookmarks(&display_bookmarks, DEFAULT_FIELDS);

    // Process each bookmark
    for bookmark in &bookmarks_to_edit {
        println!(
            "Editing: {} (ID: {})",
            bookmark.title,
            bookmark.id.unwrap_or(0)
        );

        match template_service.edit_bookmark_with_template(Some(bookmark.clone())) {
            Ok((updated_bookmark, was_modified)) => {
                if !was_modified {
                    println!("  No changes made, skipping update");
                    continue;
                }

                // Check if it's an update or a new bookmark
                if updated_bookmark.id.is_some() {
                    // Update existing bookmark
                    match bookmark_service.update_bookmark(updated_bookmark) {
                        Ok(_) => {
                            println!("  Successfully updated bookmark");
                            updated_count += 1;
                        }
                        Err(e) => println!("  Failed to update bookmark: {}", e),
                    }
                } else {
                    // Create new bookmark
                    let new_bookmark = updated_bookmark;
                    match bookmark_service.add_bookmark(
                        &new_bookmark.url,
                        Some(&new_bookmark.title),
                        Some(&new_bookmark.description),
                        Some(&new_bookmark.tags),
                        false, // Don't fetch metadata since we already have everything
                    ) {
                        Ok(_) => {
                            println!("  Successfully created new bookmark");
                            updated_count += 1;
                        }
                        Err(e) => println!("  Failed to create new bookmark: {}", e),
                    }
                }
            }
            Err(e) => println!("  Failed to edit bookmark: {}", e),
        }
    }

    println!("Updated {} bookmarks", updated_count);
    Ok(())
}

/// Delete bookmarks by their indices in the displayed list
#[instrument(skip(bookmarks), level = "debug")]
fn delete_bookmarks_by_indices(indices: Vec<i32>, bookmarks: &[Bookmark]) -> CliResult<()> {
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
#[instrument(level = "debug")]
pub fn delete_bookmarks(ids: Vec<i32>) -> CliResult<()> {
    let service = create_bookmark_service();

    // Display bookmarks to be deleted
    let mut bookmarks_to_display = Vec::new();
    for id in &ids {
        if let Ok(Some(bookmark)) = service.get_bookmark(*id) {
            bookmarks_to_display.push(DisplayBookmark::from_domain(&bookmark));
        }
    }

    if bookmarks_to_display.is_empty() {
        println!("No bookmarks found to delete");
        return Ok(());
    }

    show_bookmarks(&bookmarks_to_display, DEFAULT_FIELDS);

    // Confirm deletion
    if !confirm("Delete these bookmarks?") {
        return Err(CliError::OperationAborted);
    }

    // Sort IDs in reverse to handle database compaction correctly
    let mut sorted_ids = ids.clone();
    sorted_ids.sort_by(|a, b| b.cmp(a)); // Reverse sort

    // Delete bookmarks
    let mut deleted_count = 0;
    for id in sorted_ids {
        match service.delete_bookmark(id) {
            Ok(true) => deleted_count += 1,
            Ok(false) => eprintln!("Bookmark with ID {} not found", id),
            Err(e) => eprintln!("Error deleting bookmark with ID {}: {}", id, e),
        }
    }

    println!("Deleted {} bookmarks", deleted_count);
    Ok(())
}

#[instrument(level = "debug")]
pub fn copy_url_to_clipboard(url: &str) -> CliResult<()> {
    let clipboard_service = create_clipboard_service();

    match clipboard_service.copy_to_clipboard(url) {
        Ok(_) => {
            println!("URL copied to clipboard: {}", url);
            Ok(())
        }
        Err(e) => Err(CliError::CommandFailed(format!(
            "Failed to copy URL to clipboard: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use serial_test::serial;
    use std::collections::HashSet;

    #[test]
    #[serial]
    fn test_parse_input() {
        let input = "  1 2,  3  ";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["1", "2", "3"]);

        let input = "p 1,2,3";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["p", "1", "2", "3"]);
    }

    #[test]
    #[serial]
    fn test_get_bookmark_by_index() {
        // Create test bookmarks
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark1 = Bookmark {
            id: Some(10),
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            description: "An example site".to_string(),
            tags: tags.clone(),
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        let bookmark2 = Bookmark {
            id: Some(20),
            url: "https://test.com".to_string(),
            title: "Test".to_string(),
            description: "A test site".to_string(),
            tags,
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        let bookmarks = vec![bookmark1, bookmark2];

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
