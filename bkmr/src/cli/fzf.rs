// src/cli/fzf.rs

use std::io::Write;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::application::services::factory::{
    create_action_service, create_bookmark_service, create_clipboard_service,
    create_interpolation_service,
};
use crate::cli::error::CliResult;
use crate::cli::process::{
    clone_bookmark, copy_bookmark_url_to_clipboard, delete_bookmarks, edit_bookmarks,
    execute_bookmark_default_action,
};
use crate::domain::bookmark::Bookmark;
use crate::domain::search::SemanticSearchResult;
use crossterm::style::Stylize;
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use skim::{
    prelude::*, AnsiString, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem,
    SkimItemReceiver, SkimItemSender,
};
use tracing::{debug, instrument};
use tuikit::{attr::Attr, attr::Color, raw::IntoRawMode};

#[derive(Clone)]
struct SnippetItem {
    bookmark: Bookmark,
    display_text: String,
    preview: String,
}

impl SkimItem for SnippetItem {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.display_text)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::AnsiText(self.preview.clone())
    }

    fn output(&self) -> Cow<str> {
        Cow::Owned(self.text().to_string())
    }
}

/// Format bookmarks for enhanced display and preview
fn create_enhanced_skim_items(bookmarks: &[Bookmark]) -> Vec<Arc<dyn SkimItem>> {
    // Get action service to determine action descriptions
    let action_service = create_action_service();

    bookmarks
        .iter()
        .map(|bookmark| {
            let id = bookmark.id.unwrap_or(0).to_string();
            let action_description = action_service.get_default_action_description(bookmark);

            // Format display text with action type
            // let display_text = format!("{}: {} [{}]", id, bookmark.title, action_description);
            let display_text = format!("{}: {}", id, bookmark.title);

            // Format preview with colored headers and proper spacing
            let preview = format!(
                "{}: {}\n\n{}:\n{}\n\n{}:\n{}\n\n{}: {}",
                "Title".green().bold(),
                bookmark.title,
                "Description".yellow().bold(),
                if bookmark.description.is_empty() {
                    "No description"
                } else {
                    &bookmark.description
                },
                "URL/Content".cyan().bold(),
                bookmark.get_action_content(),
                "Default Action".magenta().bold(),
                action_description
            );

            Arc::new(SnippetItem {
                bookmark: bookmark.clone(),
                display_text,
                preview,
            }) as Arc<dyn SkimItem>
        })
        .collect()
}

impl SkimItem for Bookmark {
    fn text(&self) -> Cow<str> {
        let id = self.id.unwrap_or(0);
        let title = &self.title;
        let url = &self.url;
        let binding = self.formatted_tags();
        let tags_str = binding.trim_matches(',');

        // Get the action description
        let action_service = create_action_service();
        let action_description = action_service.get_default_action_description(self);

        // Read app settings
        let app_state = AppState::read_global();
        let fzf_opts = &app_state.settings.fzf_opts;

        // Format based on config options
        let tags_display = if fzf_opts.show_tags {
            format!(" [{}]", tags_str)
        } else {
            String::new()
        };

        // Show action description in display
        let text = if fzf_opts.no_url {
            format!("{}: {} ({}){}", id, title, action_description, tags_display)
        } else {
            format!(
                "{}: {} <{}> ({}){}",
                id, title, url, action_description, tags_display
            )
        };

        Cow::Owned(text)
    }

    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString<'a> {
        // Get the text representation
        let text = self.text();

        // Calculate indices for styling
        let id_str = self.id.unwrap_or(0).to_string();
        let title = &self.title;

        // Starting index for title (after ID and ": ")
        let start_idx_title = id_str.len() + 2;
        let end_idx_title = start_idx_title + title.len();

        // Create attribute for title (green)
        let attr_title = Attr {
            fg: Color::GREEN,
            ..Attr::default()
        };

        // Create attribute segments
        let mut attr_segments = vec![(attr_title, (start_idx_title as u32, end_idx_title as u32))];

        // Read app settings
        let app_state = AppState::read_global();
        let fzf_opts = &app_state.settings.fzf_opts;

        // If showing URL, add yellow attribute for it
        if !fzf_opts.no_url {
            let url = &self.url;
            let start_idx_url = text.find('<').unwrap_or(0) as u32;
            let end_idx_url = start_idx_url + url.len() as u32 + 2; // +2 for < and >

            let attr_url = Attr {
                fg: Color::YELLOW,
                ..Attr::default()
            };

            attr_segments.push((attr_url, (start_idx_url, end_idx_url)));
        }

        // If showing tags, add magenta attribute for tags
        if fzf_opts.show_tags && !self.tags.is_empty() {
            let start_idx_tags = text.find('[').unwrap_or(0) as u32;
            let end_idx_tags = text.find(']').unwrap_or(text.len()) as u32 + 1; // +1 for ]

            let attr_tags = Attr {
                fg: Color::MAGENTA,
                ..Attr::default()
            };

            attr_segments.push((attr_tags, (start_idx_tags, end_idx_tags)));
        }

        // Add cyan attribute for action description in parentheses
        let start_idx_action = text.rfind('(').unwrap_or(text.len()) as u32;
        let end_idx_action = text.rfind(')').unwrap_or(text.len()) as u32 + 1; // +1 for )

        let attr_action = Attr {
            fg: Color::CYAN,
            ..Attr::default()
        };

        attr_segments.push((attr_action, (start_idx_action, end_idx_action)));

        AnsiString::new_str(context.text, attr_segments)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let id = self.id.unwrap_or(0);
        let title = &self.title;
        let url = &self.url;
        let description = &self.description;
        let binding = self.formatted_tags();
        let tags = binding.trim_matches(',');

        // Get the action description
        let action_service = create_action_service();
        let action_description = action_service.get_default_action_description(self);

        let preview_text = format!(
            "ID: {}\nTitle: {}\nURL/Content: {}\nDescription: {}\nTags: {}\nAccess Count: {}\nDefault Action: {}",
            id, title, url, description, tags, self.access_count, action_description
        );

        ItemPreview::AnsiText(format!("\x1b[1mBookmark Details:\x1b[0m\n{}", preview_text))
    }
}

impl SkimItem for SemanticSearchResult {
    fn text(&self) -> Cow<str> {
        let id = self.bookmark.id.unwrap_or(0);
        let title = &self.bookmark.title;
        let url = &self.bookmark.url;
        let binding = self.bookmark.formatted_tags();
        let tags_str = binding.trim_matches(',');
        let similarity = format!("{:.1}%", self.similarity * 100.0);

        // Get the action description
        let action_service = create_action_service();
        let action_description = action_service.get_default_action_description(&self.bookmark);

        // Read app settings
        let app_state = AppState::read_global();
        let fzf_opts = &app_state.settings.fzf_opts;

        // Format based on config options
        let tags_display = if fzf_opts.show_tags {
            format!(" [{}]", tags_str)
        } else {
            String::new()
        };

        let text = if fzf_opts.no_url {
            format!(
                "{}: {} ({}%) ({}){}",
                id, title, similarity, action_description, tags_display
            )
        } else {
            format!(
                "{}: {} <{}> ({}%) ({}){}",
                id, title, url, similarity, action_description, tags_display
            )
        };

        Cow::Owned(text)
    }

    // Implement other methods...
}

/// Processes bookmarks using the fzf-like selector interface
///
/// Control keys:
/// - Enter/Ctrl-o: Execute default action for the bookmark type (open URI, copy snippet, etc.)
/// - Ctrl-e: Edit the selected bookmark
/// - Ctrl-d: Delete the selected bookmark
/// - Ctrl-a: Clone the selected bookmark
#[instrument(skip(bookmarks), level = "debug")]
pub fn fzf_process(bookmarks: &[Bookmark], style: &str) -> CliResult<()> {
    if bookmarks.is_empty() {
        eprintln!("No bookmarks to display");
        return Ok(());
    }

    // Read app settings
    let app_state = AppState::read_global();
    let fzf_opts = &app_state.settings.fzf_opts;

    // Build skim options
    let mut options_builder = SkimOptionsBuilder::default();

    // Set common options
    options_builder.height(fzf_opts.height.clone());
    options_builder.reverse(fzf_opts.reverse);
    options_builder.multi(false);
    options_builder.ansi(true);
    options_builder.filter(Some("".to_string()));

    // Add preview window only for enhanced style
    if style == "enhanced" {
        options_builder.preview(Some("".to_string()));
        options_builder.preview_window("right:70%:wrap".to_string());
    }

    // Add key bindings - updated help text to reflect default actions
    options_builder.bind(vec![
        "ctrl-a:accept".to_string(),
        "ctrl-o:accept".to_string(),
        "ctrl-y:accept".to_string(),
        "ctrl-e:accept".to_string(),
        "ctrl-d:accept".to_string(),
        "enter:accept".to_string(),
        "esc:abort".to_string(),
    ]);

    let options = options_builder.build().map_err(|e| {
        crate::cli::error::CliError::CommandFailed(format!("Failed to build skim options: {}", e))
    })?;

    // Set up channel for bookmark items
    let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();

    // Send bookmarks to skim based on style
    if style == "enhanced" {
        let skim_items = create_enhanced_skim_items(bookmarks);
        for item in skim_items {
            tx_item.send(item).map_err(|_| {
                crate::cli::error::CliError::CommandFailed(
                    "Failed to send bookmark to skim".to_string(),
                )
            })?;
        }
    } else {
        // Original style
        for bookmark in bookmarks {
            debug!("Sending bookmark to skim: {}", bookmark.title);
            tx_item.send(Arc::new(bookmark.clone())).map_err(|_| {
                crate::cli::error::CliError::CommandFailed(
                    "Failed to send bookmark to skim".to_string(),
                )
            })?;
        }
    }
    drop(tx_item); // Close channel to signal end of items

    // Execute the skim selector
    if let Some(output) = Skim::run_with(&options, Some(rx_item)) {
        let key = output.final_key;
        debug!("Final key: {:?}", key);

        // Check if the user pressed ESC - if so, don't process selected items
        if key == Key::ESC {
            debug!("Selection aborted with ESC key");
            // clear_terminal();
            clear_terminal_completely();
            return Ok(());
        }

        // Get selected bookmarks
        let selected_bookmarks = if style == "enhanced" {
            // Extract from SnippetItem
            output
                .selected_items
                .iter()
                .filter_map(|item| {
                    (**item)
                        .as_any()
                        .downcast_ref::<SnippetItem>()
                        .map(|snippet_item| snippet_item.bookmark.clone())
                })
                .collect::<Vec<Bookmark>>()
        } else {
            // Original style
            get_selected_bookmarks(&output)
        };

        if selected_bookmarks.is_empty() {
            debug!("No bookmarks selected");
            return Ok(());
        }

        // Get IDs of selected bookmarks
        let ids: Vec<i32> = selected_bookmarks.iter().filter_map(|bm| bm.id).collect();
        debug!("Selected bookmark IDs: {:?}", ids);

        // IMPORTANT: Clear the terminal completely BEFORE processing any action
        clear_terminal_completely();

        // Process the selected action based on the key
        match key {
            // Execute default action for Enter - Use the action service
            Key::Enter => {
                // clear_fzf_artifacts();
                // Execute default action for each selected bookmark
                for bookmark in &selected_bookmarks {
                    // Use the action service to execute the default action
                    execute_bookmark_default_action(bookmark)?;
                }
            }
            Key::Ctrl('y') | Key::Ctrl('o') => {
                // clear_fzf_artifacts();
                if let Some(bookmark) = selected_bookmarks.first() {
                    // Copy URL to clipboard with interpolation
                    copy_bookmark_url_to_clipboard(bookmark)?;
                }
            }
            Key::Ctrl('e') => {
                clear_fzf_artifacts();
                // Edit selected bookmarks
                edit_bookmarks(ids)?;
            }
            Key::Ctrl('d') => {
                // clear_fzf_artifacts();
                // Delete selected bookmarks
                delete_bookmarks(ids)?;
            }
            Key::Ctrl('a') => {
                // clear_fzf_artifacts();
                // Clone selected bookmark
                if let Some(bookmark) = selected_bookmarks.first() {
                    if let Some(id) = bookmark.id {
                        clone_bookmark(id)?;
                    }
                }
            }
            _ => {
                debug!("Unhandled key: {:?}", key);
            }
        }

        // Clear terminal after action
        clear_terminal();
    }

    Ok(())
}

/// Extract selected bookmarks from skim output
fn get_selected_bookmarks(output: &SkimOutput) -> Vec<Bookmark> {
    debug!("query: {:?} cmd: {:?}", output.query, output.cmd);

    let selected_bookmarks = output
        .selected_items
        .iter()
        .filter_map(|item| {
            (**item)
                .as_any()
                .downcast_ref::<Bookmark>()
                .map(|bm| bm.to_owned())
        })
        .collect::<Vec<Bookmark>>();

    if !selected_bookmarks.is_empty() {
        eprintln!("Selected bookmarks:");
        for bookmark in &selected_bookmarks {
            eprintln!(" - {}: {}", bookmark.id.unwrap_or(0), bookmark.title);
        }
    }

    debug!("Selected {} bookmarks", selected_bookmarks.len());
    selected_bookmarks
}

/// Clears the fzf interface from the terminal screen
fn clear_terminal() {
    // Try to reset terminal state without completely clearing the screen
    if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
        // Execute a sequence of terminal operations:
        // 1. Reset colors to default
        // 2. Show cursor (in case it was hidden)
        // 3. Clear from current position to end of screen (preserves any output at the top)
        if let Err(e) = execute!(
            stdout,
            crossterm::style::ResetColor,
            crossterm::cursor::Show,
            Clear(ClearType::FromCursorDown)  // this is important !!!
        ) {
            debug!("Failed to reset terminal with crossterm: {}", e);
        }

        // Ensure output is flushed
        if let Err(e) = stdout.flush() {
            debug!("Failed to flush terminal: {}", e);
        }
    }

    // Print a single newline to ensure we have a clean prompt
    println!("");
}

/// Clears fzf-specific artifacts from the terminal
/// This is less aggressive than a full clear to preserve command output
fn clear_fzf_artifacts() {
    // Gather terminal size information
    let terminal_size = crossterm::terminal::size().unwrap_or((80, 24));
    let width = terminal_size.0;

    // Print a sequence of spaces to overwrite the fzf line
    let spaces = " ".repeat(width as usize);

    // If we can get raw mode, use crossterm to position cursor and clear
    if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
        // Move to beginning of line and clear
        if let Err(e) = execute!(
            stdout,
            crossterm::cursor::MoveToColumn(0),
            crossterm::style::Print(&spaces),
            crossterm::cursor::MoveToColumn(0)
        ) {
            debug!("Failed to clear fzf line: {}", e);
        }

        // Ensure flush
        if let Err(e) = stdout.flush() {
            debug!("Failed to flush terminal: {}", e);
        }
    } else {
        // Fallback: just print a newline
        println!();
    }
}

/// Clear fzf selection UI from the terminal completely
/// This approach completely resets the terminal state to get rid of all artifacts
fn clear_terminal_completely() {
    // Try multiple approaches to ensure terminal is fully reset

    // 1. Use crossterm to attempt a full terminal reset
    if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
        // First try to clear everything and reset all attributes
        let _ = execute!(
            stdout,
            Clear(ClearType::All),
            crossterm::style::ResetColor,
            crossterm::cursor::MoveTo(0, 0),
            crossterm::cursor::Show
        );

        // Make sure changes are flushed
        let _ = stdout.flush();
    }

    // 2. As a backup, send ANSI escape codes directly
    // This sequence: clears screen, moves cursor to home position, and resets attributes
    print!("\x1B[2J\x1B[H\x1B[0m");
    std::io::stdout().flush().ok();

    // 3. If all else fails, at least print newlines to push fzf UI off the visible area
    println!("\n\n\n\n\n\n\n\n");
}
