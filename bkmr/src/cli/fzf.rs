// src/cli/fzf.rs

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use crossterm::{execute, terminal::{Clear, ClearType}};
use skim::{
    prelude::*,
    AnsiString, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem,
    SkimItemReceiver, SkimItemSender
};
use tracing::debug;
use tuikit::raw::IntoRawMode;
use crate::application::dto::BookmarkResponse;
use crate::cli::error::{CliError, CliResult};
use crate::cli::process::{edit_bookmarks, open_bookmark};
use crate::domain::bookmark::Bookmark;
use crate::domain::repositories::bookmark_repository::BookmarkRepository;
use crate::domain::tag::Tag;
use crate::environment::CONFIG;

// Wrapper struct since we can't implement external traits on external types
struct BookmarkWrapper {
    bookmark: BookmarkResponse
}

impl SkimItem for BookmarkWrapper {
    fn text(&self) -> std::borrow::Cow<str> {
        let id = self.bookmark.id.unwrap_or(0);
        let title = &self.bookmark.title;
        let url = &self.bookmark.url;
        let tags = self.bookmark.tags.join(",");

        // Format based on config options
        let tags_display = if CONFIG.fzf_opts.show_tags {
            format!(" [{}]", tags)
        } else {
            String::new()
        };

        let text = if CONFIG.fzf_opts.no_url {
            format!("{}: {}{}", id, title, tags_display)
        } else {
            format!("{}: {} <{}>{}", id, title, url, tags_display)
        };

        std::borrow::Cow::Owned(text)
    }

    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString<'a> {
        // Get the text representation
        let text = self.text();

        // Calculate indices for styling
        let id_str = self.bookmark.id.unwrap_or(0).to_string();
        let title = &self.bookmark.title;

        // Starting index for title (after ID and ": ")
        let start_idx_title = id_str.len() + 2;
        let end_idx_title = start_idx_title + title.len();

        // Create attribute for title (green)
        let attr_title = tuikit::attr::Attr {
            fg: tuikit::attr::Color::GREEN,
            ..tuikit::attr::Attr::default()
        };

        // Create attribute segments
        let mut attr_segments = vec![
            (attr_title, (start_idx_title as u32, end_idx_title as u32))
        ];

        // If showing URL, add yellow attribute for it
        if !CONFIG.fzf_opts.no_url {
            let url = &self.bookmark.url;
            let start_idx_url = text.find('<').unwrap_or(0) as u32;
            let end_idx_url = start_idx_url + url.len() as u32 + 2; // +2 for < and >

            let attr_url = tuikit::attr::Attr {
                fg: tuikit::attr::Color::YELLOW,
                ..tuikit::attr::Attr::default()
            };

            attr_segments.push((attr_url, (start_idx_url, end_idx_url)));
        }

        // If showing tags, add magenta attribute for tags
        if CONFIG.fzf_opts.show_tags && !self.bookmark.tags.is_empty() {
            let start_idx_tags = text.find('[').unwrap_or(0) as u32;
            let end_idx_tags = text.find(']').unwrap_or(text.len()) as u32 + 1; // +1 for ]

            let attr_tags = tuikit::attr::Attr {
                fg: tuikit::attr::Color::MAGENTA,
                ..tuikit::attr::Attr::default()
            };

            attr_segments.push((attr_tags, (start_idx_tags, end_idx_tags)));
        }

        AnsiString::new_str(&context.text, attr_segments)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let id = self.bookmark.id.unwrap_or(0);
        let title = &self.bookmark.title;
        let url = &self.bookmark.url;
        let description = &self.bookmark.description;
        let tags = self.bookmark.tags.join(", ");

        let preview_text = format!(
            "ID: {}\nTitle: {}\nURL: {}\nDescription: {}\nTags: {}\nAccess Count: {}",
            id, title, url, description, tags, self.bookmark.access_count
        );

        ItemPreview::AnsiText(format!("\x1b[1mBookmark Details:\x1b[0m\n{}", preview_text))
    }
}

pub fn fzf_process(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    if bookmarks.is_empty() {
        println!("No bookmarks to display");
        return Ok(());
    }

    // Build skim options
    let options = SkimOptionsBuilder::default()
        .height(CONFIG.fzf_opts.height.clone())
        .reverse(CONFIG.fzf_opts.reverse)
        .multi(true)
        .ansi(true)
        .bind(vec![
            "ctrl-o:accept".to_string(),
            "ctrl-e:accept".to_string(),
            "ctrl-d:accept".to_string(),
            "enter:accept".to_string(),
        ])
        .build()
        .expect("Failed to build skim options");

    // Set up channel for bookmark items
    let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();

    // Send bookmarks to skim
    for bookmark in bookmarks {
        let wrapper = BookmarkWrapper { bookmark: bookmark.clone() };
        tx_item.send(Arc::new(wrapper)).expect("Failed to send bookmark to skim");
    }
    drop(tx_item); // Close channel to signal end of items

    // Execute the skim selector
    if let Some(output) = Skim::run_with(&options, Some(rx_item)) {
        let key = output.final_key;

        // Extract selected bookmarks
        let selected_bookmarks = output.selected_items
            .iter()
            .filter_map(|selected| {
                selected.as_any()
                    .downcast_ref::<BookmarkWrapper>()
                    .map(|wrapper| wrapper.bookmark.clone())
            })
            .collect::<Vec<BookmarkResponse>>();

        if selected_bookmarks.is_empty() {
            return Ok(());
        }

        // Get IDs of selected bookmarks
        let ids: Vec<i32> = selected_bookmarks
            .iter()
            .filter_map(|bm| bm.id)
            .collect();

        match key {
            Key::Enter | Key::Ctrl('o') => {
                // Open selected bookmarks
                for bookmark in &selected_bookmarks {
                    open_bookmark(bookmark)?;
                }
            },
            Key::Ctrl('e') => {
                // Edit selected bookmarks
                // Convert BookmarkResponse to domain Bookmark for edit_bookmarks
                let domain_bookmarks = selected_bookmarks.iter()
                    .filter_map(|resp| {
                        if let Some(id) = resp.id {
                            // Get repository and find the domain bookmark
                            match crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository::from_url(&CONFIG.db_url) {
                                Ok(repo) => {
                                    match repo.get_by_id(id) {
                                        Ok(Some(bookmark)) => Some(bookmark),
                                        _ => None,
                                    }
                                },
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<Bookmark>>();

                edit_bookmarks(ids.clone())?;
            },
            Key::Ctrl('d') => {
                // Delete selected bookmarks
                if confirm_deletion(&ids)? {
                    let service = crate::application::services::bookmark_application_service::BookmarkApplicationService::new(
                        crate::infrastructure::repositories::sqlite::bookmark_repository::SqliteBookmarkRepository::from_url(&CONFIG.db_url)
                            .map_err(|e| CliError::RepositoryError(format!("Failed to create repository: {}", e)))?
                    );

                    for id in ids {
                        if let Err(e) = service.delete_bookmark(id) {
                            eprintln!("Failed to delete bookmark {}: {}", id, e);
                        }
                    }

                    println!("Bookmarks deleted successfully");
                }
            },
            _ => {} // Other keys are ignored
        }

        // Clear terminal after action
        if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
            execute!(stdout, Clear(ClearType::FromCursorDown)).ok();
        }
    }

    Ok(())
}

fn confirm_deletion(ids: &[i32]) -> CliResult<bool> {
    print!("Delete {} bookmark(s)? (y/N): ", ids.len());
    std::io::stdout().flush().map_err(CliError::Io)?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(CliError::Io)?;

    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}