// src/cli/fzf.rs

use std::sync::Arc;

use crate::app_state::AppState;
use crate::application::services::factory::{
    create_bookmark_service, create_clipboard_service, create_interpolation_service,
};
use crate::cli::error::CliResult;
use crate::cli::process::{delete_bookmarks, edit_bookmarks, open_bookmark};
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
    bookmarks
        .iter()
        .map(|bookmark| {
            let id = bookmark.id.unwrap_or(0).to_string();

            // Format display text
            let display_text = format!("{}: {}", id, bookmark.title);

            // Format preview with colored headers and proper spacing
            let preview = format!(
                "{}: {}\n\n{}:\n{}\n\n{}:\n{}",
                "Title".green().bold(),
                bookmark.title,
                "Description".yellow().bold(),
                if bookmark.description.is_empty() {
                    "No description"
                } else {
                    &bookmark.description
                },
                "URL".cyan().bold(),
                bookmark.url
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
            format!("{}: {}{}", id, title, tags_display)
        } else {
            format!("{}: {} <{}>{}", id, title, url, tags_display)
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

        AnsiString::new_str(context.text, attr_segments)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let id = self.id.unwrap_or(0);
        let title = &self.title;
        let url = &self.url;
        let description = &self.description;
        let binding = self.formatted_tags();
        let tags = binding.trim_matches(',');

        let preview_text = format!(
            "ID: {}\nTitle: {}\nURL: {}\nDescription: {}\nTags: {}\nAccess Count: {}",
            id, title, url, description, tags, self.access_count
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
            format!("{}: {} ({}){}", id, title, similarity, tags_display)
        } else {
            format!(
                "{}: {} <{}> ({}){}",
                id, title, url, similarity, tags_display
            )
        };

        Cow::Owned(text)
    }

    // Implement other methods...
}

/// Processes bookmarks using the fzf-like selector interface
///
/// Control keys:
/// - Enter/Ctrl-o: Open the selected bookmark
/// - Ctrl-e: Edit the selected bookmark
/// - Ctrl-d: Delete the selected bookmark
#[instrument(skip(bookmarks), level = "debug")]
pub fn fzf_process(bookmarks: &[Bookmark], style: &str) -> CliResult<()> {
    if bookmarks.is_empty() {
        println!("No bookmarks to display");
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

    // Add key bindings
    options_builder.bind(vec![
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
            clear_terminal();
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

        // Process the selected action based on the key
        match key {
            Key::Enter => {
                // Open selected bookmarks
                for bookmark in &selected_bookmarks {
                    open_bookmark(bookmark)?;
                }
            }
            Key::Ctrl('y') | Key::Ctrl('o') => {
                if let Some(bookmark) = selected_bookmarks.first() {
                    if let Some(id) = bookmark.id {
                        let service = create_bookmark_service();
                        service.record_bookmark_access(id)?;
                    }
                    // Create interpolation service and render URL
                    let interpolation_service = create_interpolation_service();
                    let rendered_url = interpolation_service.render_bookmark_url(bookmark)?;

                    let clipboard_service = create_clipboard_service();
                    clipboard_service.copy_to_clipboard(rendered_url.as_str())?;
                }
            }
            Key::Ctrl('e') => {
                // Edit selected bookmarks
                edit_bookmarks(ids)?;
            }
            Key::Ctrl('d') => {
                // Delete selected bookmarks
                delete_bookmarks(ids)?;
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
        println!("Selected bookmarks:");
        for bookmark in &selected_bookmarks {
            println!(" - {}: {}", bookmark.id.unwrap_or(0), bookmark.title);
        }
    }

    debug!("Selected {} bookmarks", selected_bookmarks.len());
    selected_bookmarks
}

/// Clears the terminal screen after an action
fn clear_terminal() {
    if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
        if let Err(e) = execute!(stdout, Clear(ClearType::FromCursorDown)) {
            debug!("Failed to clear terminal: {}", e);
        }
    }
}
