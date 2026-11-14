// src/cli/process.rs

use regex::Regex;
use std::io::{self, Write};
use std::sync::Arc;
use tracing::{debug, instrument};

use crate::application::services::action_service::ActionService;
use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::interpolation_service::InterpolationService;
use crate::application::services::template_service::TemplateService;
use crate::cli::bookmark_commands::format_action_description;
use crate::cli::display::{
    lookup_embedded_ids, show_bookmarks, DisplayBookmark, ALL_FIELDS, DEFAULT_FIELDS,
};
use crate::cli::error::{CliError, CliResult};
use crate::domain::bookmark::Bookmark;
use crate::domain::services::clipboard::ClipboardService;
use crate::infrastructure::di::ServiceContainer;
use crate::util::helper::{confirm, ensure_int_vector};

/// Process a list of bookmarks interactively
#[instrument(skip_all, level = "debug")]
pub fn process(
    bookmarks: &[Bookmark],
    services: &ServiceContainer,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    if bookmarks.is_empty() {
        return Ok(());
    }

    let help_text = r#"
       <n1> <n2>:      performs default action on selection (open URI, copy snippet, etc.)
       p <n1> <n2>:    print id-list of selection
       p:              print all ids
       d <n1> <n2>:    delete selection
       e <n1> <n2>:    edit selection
       t <n1> <n2>:    record an access (access count, last accessed)
       y <n1> <n2>:    yank/copy URL(s) to clipboard
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
                    delete_bookmarks_by_indices(
                        indices,
                        bookmarks,
                        services.bookmark_service.clone(),
                        settings,
                    )?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "e" => {
                if tokens.len() == 1 {
                    // Just "e" command with no indices - edit all bookmarks
                    edit_all_bookmarks(
                        bookmarks,
                        services.bookmark_service.clone(),
                        services.template_service.clone(),
                        settings,
                    )?;
                } else if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    edit_bookmarks_by_indices(
                        indices,
                        bookmarks,
                        services.bookmark_service.clone(),
                        services.template_service.clone(),
                        settings,
                    )?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "t" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    touch_bookmarks_by_indices(
                        indices,
                        bookmarks,
                        services.bookmark_service.clone(),
                        settings,
                    )?;
                } else {
                    eprintln!("Invalid input, only numbers allowed");
                    continue;
                }
                break;
            }
            "y" => {
                if let Some(indices) = ensure_int_vector(&tokens[1..]) {
                    yank_bookmark_urls_by_indices(
                        indices,
                        bookmarks,
                        services.interpolation_service.clone(),
                        services.clipboard_service.clone(),
                        services.action_service.clone(),
                    )?;
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
                    execute_default_actions_by_indices(
                        indices,
                        bookmarks,
                        services.action_service.clone(),
                    )?;
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

#[instrument(
    skip(bookmarks, interpolation_service, clipboard_service, action_service),
    level = "debug"
)]
fn yank_bookmark_urls_by_indices(
    indices: Vec<i32>,
    bookmarks: &[Bookmark],
    interpolation_service: Arc<dyn InterpolationService>,
    clipboard_service: Arc<dyn ClipboardService>,
    action_service: Arc<dyn ActionService>,
) -> CliResult<()> {
    debug!(
        "Yanking (copying) URLs for bookmarks at indices: {:?}",
        indices
    );

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                action_service.record_access(bookmark);

                // Render the URL with interpolation variables if needed
                let rendered_url = match interpolation_service.render_bookmark_url(bookmark) {
                    Ok(url) => url,
                    Err(e) => {
                        eprintln!("Error rendering URL for bookmark {}: {}", index, e);
                        continue;
                    }
                };

                // Copy to clipboard
                match clipboard_service.copy_to_clipboard(&rendered_url) {
                    Ok(_) => eprintln!("Copied to clipboard: {}", rendered_url),
                    Err(e) => eprintln!("Error copying to clipboard: {}", e),
                }
            }
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

/// Executes the default action for a bookmark
#[instrument(skip(action_service), level = "debug")]
pub fn execute_bookmark_default_action(
    bookmark: &Bookmark,
    action_service: Arc<dyn ActionService>,
) -> CliResult<()> {
    // Get action description for logging, incorporating custom opener if present
    let base_description = action_service.get_default_action_description(bookmark);
    let action_description = format_action_description(base_description, bookmark.opener.as_ref());
    debug!(
        "Executing default action: {} for bookmark: {}",
        action_description, bookmark.title
    );

    // Execute the default action
    // Terminal has already been cleared before this function is called
    action_service.execute_default_action(bookmark)?;

    Ok(())
}

/// Executes default actions for bookmarks by their indices
#[instrument(skip(bookmarks, action_service), level = "debug")]
fn execute_default_actions_by_indices(
    indices: Vec<i32>,
    bookmarks: &[Bookmark],
    action_service: Arc<dyn ActionService>,
) -> CliResult<()> {
    debug!(
        "Executing default actions for bookmarks at indices: {:?}",
        indices
    );

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                // Determine the action type, incorporating custom opener if present
                let base_description = action_service.get_default_action_description(bookmark);
                let action_type =
                    format_action_description(base_description, bookmark.opener.as_ref());

                // Show what we're doing
                eprintln!(
                    "Executing '{}' for bookmark: {} (ID: {})",
                    action_type,
                    bookmark.title,
                    bookmark.id.unwrap_or(0)
                );

                // Execute the action
                execute_bookmark_default_action(bookmark, action_service.clone())?
            }
            None => eprintln!("Index {} out of range", index),
        }
    }

    Ok(())
}

// For backward compatibility
#[instrument(skip(action_service), level = "debug")]
pub fn open_bookmark(bookmark: &Bookmark, action_service: Arc<dyn ActionService>) -> CliResult<()> {
    // This now delegates to the default action system
    execute_bookmark_default_action(bookmark, action_service)
}

/// Records an access for bookmarks by indices, without running their action
#[instrument(skip(bookmarks, bookmark_service, settings), level = "debug")]
fn touch_bookmarks_by_indices(
    indices: Vec<i32>,
    bookmarks: &[Bookmark],
    bookmark_service: Arc<dyn BookmarkService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    debug!("Touching bookmarks at indices: {:?}", indices);

    for index in indices {
        match get_bookmark_by_index(index, bookmarks) {
            Some(bookmark) => {
                if let Some(id) = bookmark.id {
                    bookmark_service
                        .record_bookmark_access(id)
                        .map_err(CliError::Application)?;

                    // Display the updated bookmark
                    if let Ok(Some(updated)) = bookmark_service.get_bookmark(id) {
                        show_bookmarks(
                            &[DisplayBookmark::from_domain(
                                &updated,
                                lookup_embedded_ids(&*bookmark_service).as_ref(),
                            )],
                            ALL_FIELDS,
                            settings,
                        );
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
        eprintln!("No bookmark IDs found for the specified indices");
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
        eprintln!("No bookmark IDs found");
        io::stdout().flush().map_err(CliError::Io)?; // todo: check if this is needed
        return Ok(());
    }

    // Print the count for verification
    eprintln!("Found {} bookmark IDs", ids.len());

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

/// Edit all bookmarks in the list
#[instrument(
    skip(bookmarks, bookmark_service, template_service, settings),
    level = "debug"
)]
fn edit_all_bookmarks(
    bookmarks: &[Bookmark],
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    // Get IDs from all bookmarks
    let mut bookmark_ids = Vec::new();
    for bookmark in bookmarks {
        if let Some(id) = bookmark.id {
            bookmark_ids.push(id);
        }
    }

    if bookmark_ids.is_empty() {
        eprintln!("No bookmarks to edit");
        return Ok(());
    }

    // Call the edit function with all IDs
    edit_bookmarks(
        bookmark_ids,
        false,
        bookmark_service,
        template_service,
        settings,
    )
}

/// Edit bookmarks by their indices
#[instrument(
    skip(bookmarks, bookmark_service, template_service, settings),
    level = "debug"
)]
fn edit_bookmarks_by_indices(
    indices: Vec<i32>,
    bookmarks: &[Bookmark],
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
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
    edit_bookmarks(
        bookmark_ids,
        false,
        bookmark_service,
        template_service,
        settings,
    )
}

/// Edit bookmarks by IDs
#[instrument(skip(bookmark_service, template_service, settings), level = "debug")]
pub fn edit_bookmarks(
    ids: Vec<i32>,
    force_db: bool,
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    let mut bookmarks_to_edit = Vec::new();
    let mut updated_count = 0;

    // Fetch bookmarks
    for id in &ids {
        if let Ok(Some(bookmark)) = bookmark_service.get_bookmark(*id) {
            bookmarks_to_edit.push(bookmark);
        } else {
            eprintln!("Bookmark with ID {} not found", id);
        }
    }

    if bookmarks_to_edit.is_empty() {
        eprintln!("No bookmarks found to edit");
        return Ok(());
    }

    // Display bookmarks before editing
    let embedded_ids = lookup_embedded_ids(&*bookmark_service);
    let display_bookmarks: Vec<_> = bookmarks_to_edit
        .iter()
        .map(|bookmark| DisplayBookmark::from_domain(bookmark, embedded_ids.as_ref()))
        .collect();

    show_bookmarks(&display_bookmarks, DEFAULT_FIELDS, settings);

    // Process each bookmark with smart edit strategy
    for bookmark in &bookmarks_to_edit {
        eprintln!(
            "Editing: {} (ID: {})",
            bookmark.title,
            bookmark.id.unwrap_or(0)
        );

        // Smart edit strategy: decide whether to edit source file or database content
        if !force_db && bookmark.file_path.is_some() {
            // Edit source file directly for file-imported bookmarks
            if let Err(e) = edit_source_file_and_sync(bookmark, &bookmark_service, settings) {
                eprintln!("  Failed to edit source file: {}", e);
                eprintln!("  Falling back to database content editing...");
                // Fall back to regular database editing
                if let Err(e2) =
                    edit_database_content(bookmark, &template_service, &bookmark_service)
                {
                    eprintln!("  Failed to edit database content: {}", e2);
                } else {
                    updated_count += 1;
                }
            } else {
                updated_count += 1;
            }
        } else {
            // Edit database content for regular bookmarks or when forced
            if let Err(e) = edit_database_content(bookmark, &template_service, &bookmark_service) {
                eprintln!("  Failed to edit bookmark: {}", e);
            } else {
                updated_count += 1;
            }
        }
    }

    eprintln!("Updated {} bookmarks", updated_count);
    Ok(())
}

/// Delete bookmarks by their indices in the displayed list
#[instrument(skip(bookmarks, bookmark_service, settings), level = "debug")]
fn delete_bookmarks_by_indices(
    indices: Vec<i32>,
    bookmarks: &[Bookmark],
    bookmark_service: Arc<dyn BookmarkService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
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
    delete_bookmarks(bookmark_ids, bookmark_service, settings)
}

/// Delete bookmarks by their IDs
#[instrument(skip(bookmark_service, settings), level = "debug")]
pub fn delete_bookmarks(
    ids: Vec<i32>,
    bookmark_service: Arc<dyn BookmarkService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    // Display bookmarks to be deleted
    let embedded_ids = lookup_embedded_ids(&*bookmark_service);
    let mut bookmarks_to_display = Vec::new();
    for id in &ids {
        if let Ok(Some(bookmark)) = bookmark_service.get_bookmark(*id) {
            bookmarks_to_display.push(DisplayBookmark::from_domain(
                &bookmark,
                embedded_ids.as_ref(),
            ));
        }
    }

    if bookmarks_to_display.is_empty() {
        eprintln!("No bookmarks found to delete");
        return Ok(());
    }

    show_bookmarks(&bookmarks_to_display, DEFAULT_FIELDS, settings);

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
        match bookmark_service.delete_bookmark(id) {
            Ok(true) => deleted_count += 1,
            Ok(false) => eprintln!("Bookmark with ID {} not found", id),
            Err(e) => eprintln!("Error deleting bookmark with ID {}: {}", id, e),
        }
    }

    eprintln!("Deleted {} bookmarks", deleted_count);
    Ok(())
}

#[instrument(skip(clipboard_service), level = "debug")]
pub fn copy_url_to_clipboard(
    url: &str,
    clipboard_service: Arc<dyn ClipboardService>,
) -> CliResult<()> {
    match clipboard_service.copy_to_clipboard(url) {
        Ok(_) => {
            eprintln!("Copied to clipboard: {}", url);
            Ok(())
        }
        Err(e) => Err(CliError::CommandFailed(format!(
            "Failed to copy URL to clipboard: {}",
            e
        ))),
    }
}

#[instrument(skip(interpolation_service, clipboard_service), level = "debug")]
pub fn copy_bookmark_url_to_clipboard(
    bookmark: &Bookmark,
    interpolation_service: Arc<dyn InterpolationService>,
    clipboard_service: Arc<dyn ClipboardService>,
) -> CliResult<()> {
    // Render the URL (apply interpolation)
    let rendered_url = interpolation_service
        .render_bookmark_url(bookmark)
        .map_err(|e| CliError::CommandFailed(format!("Failed to render URL: {}", e)))?;

    // Copy the rendered URL to clipboard
    copy_url_to_clipboard(&rendered_url, clipboard_service)
}

/// Clone a bookmark by ID, opening the editor to modify it before saving
#[instrument(skip(bookmark_service, template_service), level = "debug")]
pub fn clone_bookmark(
    id: i32,
    bookmark_service: Arc<dyn BookmarkService>,
    template_service: Arc<dyn TemplateService>,
) -> CliResult<()> {
    // Get the bookmark to clone
    let bookmark = bookmark_service
        .get_bookmark(id)?
        .ok_or_else(|| CliError::CommandFailed(format!("No bookmark found with ID {}", id)))?;

    println!(
        "Cloning bookmark: {} (ID: {})",
        bookmark.title,
        bookmark.id.unwrap_or(0)
    );

    // Create a template with the bookmark data but WITHOUT ID
    let mut temp_bookmark = bookmark.clone();
    // Clear the ID to ensure a new bookmark will be created
    temp_bookmark.id = None;

    // Open the editor with the prepared template
    match template_service.edit_bookmark_with_template(Some(temp_bookmark)) {
        Ok((edited_bookmark, was_modified)) => {
            if !was_modified {
                println!("No changes made in editor. Bookmark not cloned.");
                return Ok(());
            }

            // Add the edited bookmark as a new bookmark
            match bookmark_service.add_bookmark(
                &edited_bookmark.url,
                Some(&edited_bookmark.title),
                Some(&edited_bookmark.description),
                Some(&edited_bookmark.tags),
                false, // Don't fetch metadata since we've already edited it
                Some(edited_bookmark.embeddable),
                edited_bookmark.opener.as_deref(),
            ) {
                Ok(new_bookmark) => {
                    println!(
                        "Added cloned bookmark: {} (ID: {})",
                        new_bookmark.title,
                        new_bookmark.id.unwrap_or(0)
                    );
                }
                Err(e) => {
                    return Err(CliError::CommandFailed(format!(
                        "Failed to add cloned bookmark: {}",
                        e
                    )));
                }
            }
        }
        Err(e) => {
            return Err(CliError::CommandFailed(format!(
                "Failed to edit bookmark: {}",
                e
            )));
        }
    }

    Ok(())
}

/// Edit source file directly and sync changes back to database
fn edit_source_file_and_sync(
    bookmark: &Bookmark,
    bookmark_service: &Arc<dyn crate::application::services::bookmark_service::BookmarkService>,
    settings: &crate::config::Settings,
) -> CliResult<()> {
    use crate::config::resolve_file_path;
    use std::path::Path;
    use std::process::Command;

    // Get the file path
    let file_path_str = bookmark
        .file_path
        .as_ref()
        .ok_or_else(|| CliError::CommandFailed("No file path for this bookmark".to_string()))?;

    // Resolve the file path (handle base path variables and environment variables)
    let resolved_path = resolve_file_path(&settings.base_paths, file_path_str);
    let source_file = Path::new(&resolved_path);

    // Check if file exists
    if !source_file.exists() {
        return Err(CliError::CommandFailed(format!(
            "Source file does not exist: {}",
            resolved_path
        )));
    }

    eprintln!("  Editing source file: {}", resolved_path);

    // Get the editor command
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    // Edit the file with the user's preferred editor
    let status = Command::new(&editor)
        .arg(&resolved_path)
        .status()
        .map_err(|e| {
            CliError::CommandFailed(format!("Failed to start editor '{}': {}", editor, e))
        })?;

    if !status.success() {
        return Err(CliError::CommandFailed(format!(
            "Editor '{}' exited with non-zero status",
            editor
        )));
    }

    eprintln!("  File edited successfully, syncing changes to database...");

    // Store what the file says now, the way import-files --update does
    let id = bookmark
        .id
        .ok_or_else(|| CliError::CommandFailed("Bookmark has no ID".to_string()))?;
    match bookmark_service.sync_from_file(id) {
        Ok(_) => {
            eprintln!("  Successfully synced changes to database");
        }
        Err(e) => {
            return Err(CliError::CommandFailed(format!(
                "Failed to sync file changes to database: {}",
                e
            )));
        }
    }

    Ok(())
}

/// Edit database content using the traditional template-based approach
fn edit_database_content(
    bookmark: &Bookmark,
    template_service: &Arc<dyn crate::application::services::template_service::TemplateService>,
    bookmark_service: &Arc<dyn crate::application::services::bookmark_service::BookmarkService>,
) -> CliResult<()> {
    match template_service.edit_bookmark_with_template(Some(bookmark.clone())) {
        Ok((updated_bookmark, was_modified)) => {
            if !was_modified {
                eprintln!("  No changes made, skipping update");
                return Ok(());
            }

            // Check if it's an update or a new bookmark
            if updated_bookmark.id.is_some() {
                // Update existing bookmark
                match bookmark_service.update_bookmark(updated_bookmark, false) {
                    Ok(_) => {
                        eprintln!("  Successfully updated bookmark");
                    }
                    Err(e) => return Err(CliError::Application(e)),
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
                    Some(new_bookmark.embeddable),
                    new_bookmark.opener.as_deref(),
                ) {
                    Ok(_) => {
                        eprintln!("  Successfully created new bookmark");
                    }
                    Err(e) => return Err(CliError::Application(e)),
                }
            }
        }
        Err(e) => return Err(CliError::Application(e)),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::util::testing::init_test_env;
    use std::collections::HashSet;

    #[test]
    fn given_input_string_when_parse_input_then_returns_sorted_tokens() {
        let input = "  1 2,  3  ";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["1", "2", "3"]);

        let input = "p 1,2,3";
        let tokens = parse_input(input);
        assert_eq!(tokens, vec!["p", "1", "2", "3"]);
    }

    #[test]
    fn given_bookmarks_and_valid_index_when_get_bookmark_by_index_then_returns_bookmark() {
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
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
            accessed_at: None,
            verified_at: None,
        };

        let bookmark2 = Bookmark {
            id: Some(20),
            url: "https://test.com".to_string(),
            title: "Test".to_string(),
            description: "A test site".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
            accessed_at: None,
            verified_at: None,
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

    #[test]
    fn given_bookmarks_and_indices_when_yank_urls_then_copies_urls_to_clipboard() {
        // Arrange
        let _ = init_test_env();

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
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
            accessed_at: None,
            verified_at: None,
        };

        let bookmark2 = Bookmark {
            id: Some(20),
            url: "https://test.com".to_string(),
            title: "Test".to_string(),
            description: "A test site".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
            accessed_at: None,
            verified_at: None,
        };

        let bookmarks = vec![bookmark1, bookmark2];

        // Act - test that the function executes without errors
        // We can't easily test clipboard content in unit tests
        // Create a temporary test service container for testing
        use crate::util::test_service_container::TestServiceContainer;
        let services = TestServiceContainer::new();

        let result = yank_bookmark_urls_by_indices(
            vec![1],
            &bookmarks,
            services.interpolation_service.clone(),
            services.clipboard_service.clone(),
            services.action_service.clone(),
        );

        // Assert
        assert!(result.is_ok(), "Yank operation should succeed");
    }

    #[test]
    fn given_stored_bookmark_when_yank_then_records_access() {
        use crate::infrastructure::clipboard::DummyClipboardService;
        use crate::util::test_service_container::TestServiceContainer;

        let services = TestServiceContainer::new();
        let bookmark = services
            .bookmark_service
            .add_bookmark(
                "prompt yank content",
                Some("Prompt yank"),
                None,
                None,
                false,
                Some(false),
                None,
            )
            .unwrap();

        yank_bookmark_urls_by_indices(
            vec![1],
            std::slice::from_ref(&bookmark),
            services.interpolation_service.clone(),
            Arc::new(DummyClipboardService::new()),
            services.action_service.clone(),
        )
        .unwrap();

        let stored = services
            .bookmark_service
            .get_bookmark(bookmark.id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(stored.access_count, 1);
    }
}
