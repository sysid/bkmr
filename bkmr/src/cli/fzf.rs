// src/cli/fzf.rs

use std::any::Any;
use std::io::Write;
use std::sync::Arc;

use crate::application::dto::BookmarkResponse;
use crate::cli::error::CliResult;
use crate::cli::process::{delete_bookmarks, edit_bookmarks, open_bookmark};
use crate::environment::CONFIG;
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use skim::{
    prelude::*, AnsiString, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem,
    SkimItemReceiver, SkimItemSender,
};
use tracing::{debug, instrument};
use tuikit::raw::IntoRawMode;

// Wrapper struct since we can't implement external traits on external types
#[derive(Clone, Debug)]
struct BookmarkWrapper {
    bookmark: BookmarkResponse,
}

impl SkimItem for BookmarkResponse {
    fn text(&self) -> std::borrow::Cow<str> {
        let id = self.id.unwrap_or(0);
        let title = &self.title;
        let url = &self.url;
        let tags_str = self.tags.join(",");

        // Format based on config options
        let tags_display = if CONFIG.fzf_opts.show_tags {
            format!(" [{}]", tags_str)
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
        let id_str = self.id.unwrap_or(0).to_string();
        let title = &self.title;

        // Starting index for title (after ID and ": ")
        let start_idx_title = id_str.len() + 2;
        let end_idx_title = start_idx_title + title.len();

        // Create attribute for title (green)
        let attr_title = tuikit::attr::Attr {
            fg: tuikit::attr::Color::GREEN,
            ..tuikit::attr::Attr::default()
        };

        // Create attribute segments
        let mut attr_segments = vec![(attr_title, (start_idx_title as u32, end_idx_title as u32))];

        // If showing URL, add yellow attribute for it
        if !CONFIG.fzf_opts.no_url {
            let url = &self.url;
            let start_idx_url = text.find('<').unwrap_or(0) as u32;
            let end_idx_url = start_idx_url + url.len() as u32 + 2; // +2 for < and >

            let attr_url = tuikit::attr::Attr {
                fg: tuikit::attr::Color::YELLOW,
                ..tuikit::attr::Attr::default()
            };

            attr_segments.push((attr_url, (start_idx_url, end_idx_url)));
        }

        // If showing tags, add magenta attribute for tags
        if CONFIG.fzf_opts.show_tags && !self.tags.is_empty() {
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
        let id = self.id.unwrap_or(0);
        let title = &self.title;
        let url = &self.url;
        let description = &self.description;
        let tags = self.tags.join(", ");

        let preview_text = format!(
            "ID: {}\nTitle: {}\nURL: {}\nDescription: {}\nTags: {}\nAccess Count: {}",
            id, title, url, description, tags, self.access_count
        );

        ItemPreview::AnsiText(format!("\x1b[1mBookmark Details:\x1b[0m\n{}", preview_text))
    }
}

#[instrument(skip(bookmarks), level = "debug")]
pub fn fzf_process(bookmarks: &[BookmarkResponse]) -> CliResult<()> {
    if bookmarks.is_empty() {
        println!("No bookmarks to display");
        return Ok(());
    }

    // Build skim options
    let options = SkimOptionsBuilder::default()
        .height(CONFIG.fzf_opts.height.clone())
        .reverse(CONFIG.fzf_opts.reverse)
        .multi(false)
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

    // Send bookmarks to skim through wrapper
    for bookmark in bookmarks {
        debug!("Sending bookmark to skim: {:?}", bookmark);
        tx_item
            .send(Arc::new(bookmark.clone()))
            .expect("Failed to send bookmark to skim");
    }
    drop(tx_item); // Close channel to signal end of items

    // Execute the skim selector
    if let Some(output) = Skim::run_with(&options, Some(rx_item)) {
        let key = output.final_key;
        debug!("Final key: {:?}", key);

        // let selected_bookmarks = filter_bms(output);
        if output.selected_items.is_empty() {
            println!("No items selected.");
        } else {
            println!("Selected items:");
            for item in &output.selected_items {
                println!(" - {}", item.output());
            }
        }
        let selected_bookmarks = output
            .selected_items
            .iter()
            .map(|selected_item| {
                (**selected_item)
                    .as_any()
                    .downcast_ref::<BookmarkResponse>()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<BookmarkResponse>>();

        debug!("Selected bookmarks: {:?}", selected_bookmarks);
        //
        // if selected_bookmarks.is_empty() {
        //     return Ok(());
        // }
        //
        // // Get IDs of selected bookmarks
        // let ids: Vec<i32> = selected_bookmarks.iter().filter_map(|bm| bm.id).collect();
        // debug!("Selected bookmark IDs: {:?}", ids);
        //
        // match key {
        //     Key::Enter | Key::Ctrl('o') => {
        //         // Open selected bookmarks
        //         for bookmark in &selected_bookmarks {
        //             open_bookmark(bookmark)?;
        //         }
        //     }
        //     Key::Ctrl('e') => {
        //         // Edit selected bookmarks
        //         edit_bookmarks(ids)?;
        //     }
        //     Key::Ctrl('d') => {
        //         // Delete selected bookmarks
        //         delete_bookmarks(ids)?;
        //     }
        //     _ => {} // Other keys are ignored
        // }
        //
        // // Clear terminal after action
        // if let Ok(mut stdout) = std::io::stdout().into_raw_mode() {
        //     execute!(stdout, Clear(ClearType::FromCursorDown)).ok();
        // }
    }

    Ok(())
}
fn filter_bms(out: SkimOutput) -> Vec<BookmarkResponse> {
    debug!("query: {:?} cmd: {:?}", out.query, out.cmd);

    out.selected_items.iter().for_each(|i| {
        println!("{}\n", i.output());
    });
    let selected_bms = out
        .selected_items
        .iter()
        .map(|selected_item| {
            (**selected_item)
                .as_any()
                .downcast_ref::<BookmarkResponse>()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<BookmarkResponse>>();
    debug!("selected_bms: {:?}", selected_bms);
    selected_bms
}
