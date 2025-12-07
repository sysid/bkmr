// src/cli/fzf.rs

use std::io::Write;
use std::sync::Arc;

// Service container dependency injection implemented  
use crate::infrastructure::di::ServiceContainer;
use crate::cli::bookmark_commands;
use crate::cli::error::CliResult;
use crate::cli::process::{
    clone_bookmark, copy_bookmark_url_to_clipboard, copy_url_to_clipboard, delete_bookmarks,
    edit_bookmarks, execute_bookmark_default_action,
};
use crate::domain::bookmark::Bookmark;
use crate::domain::search::SemanticSearchResult;
use crate::domain::system_tag::SystemTag;
use crate::util::helper::{format_file_path, format_mtime};
use crossterm::style::Stylize;
use crossterm::{
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use skim::tuikit::attr::{Attr, Color};
use skim::{
    prelude::*, AnsiString, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem,
    SkimItemReceiver, SkimItemSender,
};
use tracing::{debug, instrument};

#[derive(Clone)]
struct SnippetItem {
    bookmark: Bookmark,
    display_text: String,
    preview: String,
}

impl SkimItem for SnippetItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.display_text)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::AnsiText(self.preview.clone())
    }

    fn output(&self) -> Cow<'_, str> {
        if let Some(id) = self.bookmark.id {
            Cow::Owned(id.to_string())
        } else {
            Cow::Borrowed("0")
        }
    }
}

#[derive(Clone)]
struct AlignedBookmark {
    bookmark: Bookmark,
    max_id_width: usize,
    action_description: String,
    settings: crate::config::Settings,
}

impl SkimItem for AlignedBookmark {
    fn text(&self) -> Cow<'_, str> {
        let display_text = create_bookmark_display_text(
            &self.bookmark, 
            self.max_id_width, 
            &self.action_description, 
            &self.settings
        );
        Cow::Owned(display_text)
    }

    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString<'a> {
        // Get the text representation
        let text = self.text();

        // Calculate padding width
        let padding = self.max_id_width + 2; // ID width + ": "
        let title = &self.bookmark.title;

        // Create attribute for title (green)
        let attr_title = Attr {
            fg: Color::GREEN,
            ..Attr::default()
        };

        // Create attribute segments
        let mut attr_segments =
            vec![(attr_title, (padding as u32, (padding + title.len()) as u32))];

        // Get app settings from struct
        let fzf_opts = &self.settings.fzf_opts;

        // If showing URL, add yellow attribute for it
        if !fzf_opts.no_url {
            let start_idx_url = text.find('<').unwrap_or(0) as u32;
            if start_idx_url > 0 {
                let end_idx_url = text.find('>').unwrap_or(text.len()) as u32 + 1; // +1 for >
                attr_segments.push((
                    Attr {
                        fg: Color::YELLOW,
                        ..Attr::default()
                    },
                    (start_idx_url, end_idx_url),
                ));
            }
        }

        // If showing tags, add magenta attribute for tags
        if fzf_opts.show_tags && !self.bookmark.tags.is_empty() {
            let start_idx_tags = text.find('[').unwrap_or(0) as u32;
            if start_idx_tags > 0 && start_idx_tags < text.len() as u32 {
                let end_idx_tags = text.find(']').unwrap_or(text.len()) as u32 + 1; // +1 for ]
                attr_segments.push((
                    Attr {
                        fg: Color::MAGENTA,
                        ..Attr::default()
                    },
                    (start_idx_tags, end_idx_tags),
                ));
            }
        }

        // Add cyan attribute for action description in parentheses
        if fzf_opts.show_action {
            if let (Some(start), Some(end)) = (text.rfind('('), text.rfind(')')) {
                attr_segments.push((
                    Attr {
                        fg: Color::CYAN,
                        ..Attr::default()
                    },
                    (start as u32, end as u32 + 1),
                ));
            }
        }

        // Add grey attribute for file info line (if present)
        if fzf_opts.show_file_info {
            if let Some(file_info_start) = text.find("📁") {
                // Find the end of the file info line (next newline or end of string)
                let file_info_end = text[file_info_start..]
                    .find('\n')
                    .map(|pos| file_info_start + pos)
                    .unwrap_or(text.len());

                attr_segments.push((
                    Attr {
                        fg: Color::LIGHT_BLACK, // Grey color
                        ..Attr::default()
                    },
                    (file_info_start as u32, file_info_end as u32),
                ));
            }
        }

        AnsiString::new_str(context.text, attr_segments)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let preview_text = create_bookmark_preview_text(&self.bookmark, &self.action_description);
        ItemPreview::AnsiText(preview_text)
    }

    fn output(&self) -> Cow<'_, str> {
        if let Some(id) = self.bookmark.id {
            Cow::Owned(id.to_string())
        } else {
            Cow::Borrowed("0")
        }
    }
}

/// Create display text for a bookmark with proper formatting
fn create_bookmark_display_text(
    bookmark: &Bookmark, 
    max_id_width: usize, 
    action_description: &str, 
    settings: &crate::config::Settings
) -> String {
    let id = bookmark.id.unwrap_or(0);
    let title = &bookmark.title;
    let url = &bookmark.url;
    let binding = bookmark.formatted_tags();
    let tags_str = binding.trim_matches(',');
    
    let fzf_opts = &settings.fzf_opts;
    
    // Format based on config options
    let tags_display = if fzf_opts.show_tags {
        format!(" [{}]", tags_str)
    } else {
        String::new()
    };
    
    let action_display = if fzf_opts.show_action {
        format!(" ({})", action_description)
    } else {
        String::new()
    };
    
    let mut text = if fzf_opts.no_url {
        format!(
            "{:>width$}: {}{}{}",
            id, title, action_display, tags_display,
            width = max_id_width
        )
    } else {
        format!(
            "{:>width$}: {} <{}>{}{}",
            id, title, url, action_display, tags_display,
            width = max_id_width
        )
    };
    
    // Add file info if present and enabled
    if fzf_opts.show_file_info {
        if let (Some(file_path), Some(file_mtime)) = (&bookmark.file_path, bookmark.file_mtime) {
            let padding = " ".repeat(max_id_width + 2);
            let formatted_path = format_file_path(file_path, 120);
            let formatted_time = format_mtime(file_mtime);
            text.push_str(&format!(
                "\n{}📁 {} ({})",
                padding, formatted_path, formatted_time
            ));
        }
    }
    
    text
}

/// Create preview text for a bookmark
fn create_bookmark_preview_text(bookmark: &Bookmark, action_description: &str) -> String {
    let mut preview_text = format!(
        "ID: {}\nTitle: {}\nURL/Content: {}\nDescription: {}\nTags: {}\nAccess Count: {}\nDefault Action: {}",
        bookmark.id.unwrap_or(0),
        bookmark.title,
        bookmark.url,
        bookmark.description,
        bookmark.formatted_tags().trim_matches(','),
        bookmark.access_count,
        action_description
    );

    // Add file info if present
    if let (Some(file_path), Some(file_mtime)) = (&bookmark.file_path, bookmark.file_mtime) {
        let formatted_path = format_file_path(file_path, 120);
        let formatted_time = format_mtime(file_mtime);
        preview_text.push_str(&format!(
            "\n\nSource: {} ({})",
            formatted_path, formatted_time
        ));
    }

    format!("\x1b[1mBookmark Details:\x1b[0m\n{}", preview_text)
}

/// Format bookmarks for enhanced display and preview
fn create_enhanced_skim_items(
    bookmarks: &[Bookmark],
    max_id_width: usize,
    services: &ServiceContainer,
    _show_file_info: bool,
    show_action: bool,
) -> Vec<Arc<dyn SkimItem>> {
    // Get action service to determine action descriptions
    let action_service = &services.action_service;

    // Get interpolation service to render URLs
    let interpolation_service = &services.interpolation_service;

    bookmarks
        .iter()
        .map(|bookmark| {
            let id = bookmark.id.unwrap_or(0);
            let action_description = action_service.get_default_action_description(bookmark);

            // Format display text with action type and proper alignment
            let display_text = format!("{:>width$}: {}", id, bookmark.title, width = max_id_width);

            // Apply interpolation if the URL contains template variables
            let rendered_url = if bookmark.url.contains("{{") || bookmark.url.contains("{%") {
                match interpolation_service.render_bookmark_url(bookmark) {
                    Ok(url) => url,
                    Err(_) => bookmark.url.clone(), // Fallback if rendering fails
                }
            } else {
                bookmark.url.clone()
            };

            // Format tags for display
            let tags_str = bookmark
                .formatted_tags()
                .replace(',', " ")
                .trim()
                .to_string();
            let has_tags = !tags_str.is_empty();

            // Format preview with proper spacing (simplified)
            let preview = if show_action {
                // Include the default action in preview and tags at the bottom
                let mut preview_text = format!(
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
                    rendered_url, // Use the rendered URL instead of raw URL
                    "Default Action".magenta().bold(),
                    action_description
                );

                // Add tags section if there are any tags
                if has_tags {
                    preview_text.push_str(&format!("\n\n{}: {}", "Tags".blue().bold(), tags_str));
                }

                // Add file info if present
                if let (Some(file_path), Some(file_mtime)) =
                    (&bookmark.file_path, &bookmark.file_mtime)
                {
                    let formatted_path = format_file_path(file_path, 120);
                    let formatted_time = format_mtime(*file_mtime);
                    preview_text.push_str(&format!(
                        "\n\n{}: {} ({})",
                        "Source File".dark_grey().bold(),
                        formatted_path,
                        formatted_time
                    ));
                }

                preview_text
            } else {
                // Omit the default action in preview but still include tags at the bottom
                let mut preview_text = format!(
                    "{}: {}\n\n{}:\n{}\n\n{}:\n{}",
                    "Title".green().bold(),
                    bookmark.title,
                    "Description".yellow().bold(),
                    if bookmark.description.is_empty() {
                        "No description"
                    } else {
                        &bookmark.description
                    },
                    "URL/Content".cyan().bold(),
                    rendered_url // Use the rendered URL instead of raw URL
                );

                // Add tags section if there are any tags
                if has_tags {
                    preview_text.push_str(&format!("\n\n{}: {}", "Tags".blue().bold(), tags_str));
                }

                // Add file info if present
                if let (Some(file_path), Some(file_mtime)) =
                    (&bookmark.file_path, &bookmark.file_mtime)
                {
                    let formatted_path = format_file_path(file_path, 120);
                    let formatted_time = format_mtime(*file_mtime);
                    preview_text.push_str(&format!(
                        "\n\n{}: {} ({})",
                        "Source File".dark_grey().bold(),
                        formatted_path,
                        formatted_time
                    ));
                }

                preview_text
            };

            Arc::new(SnippetItem {
                bookmark: bookmark.clone(),
                display_text,
                preview,
            }) as Arc<dyn SkimItem>
        })
        .collect()
}

// Helper function to get selected bookmarks from output
fn get_selected_bookmarks_from_aligned(
    output: &SkimOutput,
    bookmarks: &[Bookmark],
) -> Vec<Bookmark> {
    let selected_ids: Vec<i32> = output
        .selected_items
        .iter()
        .filter_map(|item| {
            // Get the output which contains the bookmark ID as a string
            let id_str = item.output();
            id_str.parse::<i32>().ok()
        })
        .collect();

    // Find the corresponding bookmarks
    let selected_bookmarks: Vec<Bookmark> = bookmarks
        .iter()
        .filter(|b| b.id.is_some() && selected_ids.contains(&b.id.unwrap()))
        .cloned()
        .collect();

    if !selected_bookmarks.is_empty() {
        eprintln!("Selected bookmarks:");
        for bookmark in &selected_bookmarks {
            eprintln!(" - {}: {}", bookmark.id.unwrap_or(0), bookmark.title);
        }
    }

    debug!("Selected {} bookmarks", selected_bookmarks.len());
    selected_bookmarks
}

impl SkimItem for SemanticSearchResult {
    fn text(&self) -> Cow<'_, str> {
        let text = self.display();
        Cow::Owned(text)
    }
}

/// Processes bookmarks using the fzf-like selector interface
#[instrument(skip(bookmarks), level = "debug")]
pub fn fzf_process(bookmarks: &[Bookmark], style: &str, services: &ServiceContainer, settings: &crate::config::Settings, stdout: bool) -> CliResult<()> {
    if bookmarks.is_empty() {
        eprintln!("No bookmarks to display");
        return Ok(());
    }

    // Sort bookmarks by title (case-insensitive)
    let mut sorted_bookmarks = bookmarks.to_vec();
    sorted_bookmarks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    // Find the maximum ID width for proper alignment
    let max_id_width = sorted_bookmarks
        .iter()
        .map(|b| b.id.unwrap_or(0).to_string().len())
        .max()
        .unwrap_or(0);

    // Use provided settings
    let fzf_opts = &settings.fzf_opts;

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

    // Add key bindings
    options_builder.bind(vec![
        "ctrl-a:accept".to_string(),
        "ctrl-o:accept".to_string(),
        "ctrl-y:accept".to_string(),
        "ctrl-e:accept".to_string(),
        "ctrl-d:accept".to_string(),
        "ctrl-p:accept".to_string(),
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
        let skim_items = create_enhanced_skim_items(&sorted_bookmarks, max_id_width, services, fzf_opts.show_file_info, fzf_opts.show_action);
        for item in skim_items {
            tx_item.send(item).map_err(|_| {
                crate::cli::error::CliError::CommandFailed(
                    "Failed to send bookmark to skim".to_string(),
                )
            })?;
        }
    } else {
        // Original style - use AlignedBookmark instead
        for bookmark in &sorted_bookmarks {
            debug!("Sending bookmark to skim: {}", bookmark.title);
            
            // Get action description
            let action_description = services.action_service.get_default_action_description(bookmark);
            
            let item = Arc::new(AlignedBookmark {
                bookmark: bookmark.clone(),
                max_id_width,
                action_description: action_description.to_string(),
                settings: settings.clone(),
            });
            tx_item.send(item).map_err(|_| {
                crate::cli::error::CliError::CommandFailed(
                    "Failed to send bookmark to skim".to_string(),
                )
            })?;
        }
    }
    drop(tx_item); // Close channel to signal end of items

    // Determine if we need to manually handle alternate screen
    // Skim uses alternate screen automatically for height=100%, but not for smaller heights
    // For height < 100%, we wrap with alternate screen to ensure proper terminal restoration
    //
    // IMPORTANT: Skip alternate screen handling in --stdout mode because these escape sequences
    // (\E[?1049h, \E[?1049l) would pollute the output that shell widgets capture.
    // In stdout mode, skim still works fine - we just accept potential terminal artifacts.
    let use_alternate_screen =
        !stdout && fzf_opts.height != "100%" && fzf_opts.height != "100";

    if use_alternate_screen {
        execute!(std::io::stdout(), EnterAlternateScreen)?;
    }

    // Execute the skim selector
    let skim_output = Skim::run_with(&options, Some(rx_item));

    // Restore terminal if we entered alternate screen
    if use_alternate_screen {
        execute!(std::io::stdout(), LeaveAlternateScreen)?;
    }

    if let Some(output) = skim_output {
        let key = output.final_key;
        debug!("Final key: {:?}", key);

        // Check if the user pressed ESC - if so, don't process selected items
        if key == Key::ESC {
            debug!("Selection aborted with ESC key");
            reset_terminal_state(stdout);
            return Ok(());
        }

        // Get selected bookmarks
        let selected_bookmarks = get_selected_bookmarks_from_aligned(&output, &sorted_bookmarks);

        if selected_bookmarks.is_empty() {
            debug!("No bookmarks selected");
            return Ok(());
        }

        // Get IDs of selected bookmarks
        let ids: Vec<i32> = selected_bookmarks.iter().filter_map(|bm| bm.id).collect();
        debug!("Selected bookmark IDs: {:?}", ids);

        // Ensure clean output positioning before processing action
        // Skim handles terminal restoration - we just need a newline for output
        println!();

        // Process the selected action based on the key
        match key {
            // Execute default action for Enter - Use the action service
            Key::Enter => {
                if stdout {
                    // Output interpolated content to stdout instead of executing
                    for bookmark in &selected_bookmarks {
                        let content = services.interpolation_service
                            .render_bookmark_url(bookmark)
                            .map_err(|e| crate::cli::error::CliError::CommandFailed(
                                format!("Failed to render content: {}", e)
                            ))?;
                        println!("{}", content);
                    }
                } else {
                    // Execute default action for each selected bookmark
                    for bookmark in &selected_bookmarks {
                        // Use the action service to execute the default action
                        execute_bookmark_default_action(bookmark, services.action_service.clone())?;
                    }
                }
            }
            Key::Ctrl('y') | Key::Ctrl('o') => {
                // clear_fzf_artifacts();
                if let Some(bookmark) = selected_bookmarks.first() {
                    // Check if this is a shell script
                    let is_shell_script = bookmark
                        .tags
                        .iter()
                        .any(|tag| tag.is_system_tag_of(SystemTag::Shell));

                    if is_shell_script {
                        // For shell scripts, copy the bkmr open command instead of URL content
                        let command =
                            format!("bkmr open --no-edit {} --", bookmark.id.unwrap_or(0));
                        copy_url_to_clipboard(&command, services.clipboard_service.clone())?;
                    } else {
                        // For all other types, copy URL to clipboard with interpolation
                        copy_bookmark_url_to_clipboard(bookmark, services.interpolation_service.clone(), services.clipboard_service.clone())?;
                    }
                }
            }
            Key::Ctrl('e') => {
                // Edit selected bookmarks - editor handles its own terminal
                edit_bookmarks(ids, false, services.bookmark_service.clone(), services.template_service.clone(), settings)?;
            }
            Key::Ctrl('d') => {
                // clear_fzf_artifacts();
                // Delete selected bookmarks
                delete_bookmarks(ids, services.bookmark_service.clone(), settings)?;
            }
            Key::Ctrl('a') => {
                // clear_fzf_artifacts();
                // Clone selected bookmark
                if let Some(bookmark) = selected_bookmarks.first() {
                    if let Some(id) = bookmark.id {
                        clone_bookmark(id, services.bookmark_service.clone(), services.template_service.clone())?;
                    }
                }
            }
            Key::Ctrl('p') => {
                // Show detailed information for the selected bookmark
                if let Some(bookmark) = selected_bookmarks.first() {
                    // Clear from cursor for clean detail view
                    let _ = execute!(
                        std::io::stdout(),
                        Clear(ClearType::FromCursorDown)
                    );

                    // Use the shared function to show bookmark details
                    let details = bookmark_commands::show_bookmark_details(bookmark, services);
                    print!("{}", details);
                }
            }
            _ => {
                debug!("Unhandled key: {:?}", key);
            }
        }

        // Reset terminal state after action (cursor visible, colors reset)
        // Skip for --stdout mode to keep output clean for shell widgets
        reset_terminal_state(stdout);
    }

    Ok(())
}

/// Minimal terminal state reset after skim exit.
///
/// This reset is useful for interactive mode as a safety measure after:
/// - `execute_bookmark_default_action()` which runs external commands/browsers
/// - `edit_bookmarks()` which opens editors that might alter terminal state
/// - `Ctrl+P` detail printing which uses colors
///
/// However, for `--stdout` mode (shell widget integration), this function
/// must be skipped because it writes ANSI escape sequences (\E[0m, \E[?25h)
/// to stdout, polluting the output that the shell widget captures.
/// In stdout mode, we're just printing content - no terminal state changes occur.
fn reset_terminal_state(skip: bool) {
    if skip {
        return;
    }
    let mut stdout = std::io::stdout();
    let _ = execute!(
        stdout,
        crossterm::style::ResetColor,
        crossterm::cursor::Show
    );
    let _ = stdout.flush();
}

