use arboard::Clipboard;
use itertools::Itertools;
use std::borrow::Cow;
use std::sync::Arc;

use crossterm::{
    execute,
    terminal::{Clear, ClearType},
};
use skim::prelude::*;
use skim::{
    AnsiString, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem, SkimItemReceiver,
    SkimItemSender,
};
use tracing::debug;
use tuikit::prelude::*;

use crate::environment::{FzfEnvOpts, CONFIG};
use crate::model::bookmark::Bookmark;
use crate::model::tag::Tags;
use crate::service::process::{delete_bms, edit_bms, open_bms};

impl SkimItem for Bookmark {
    fn text(&self) -> Cow<str> {
        let FzfEnvOpts {
            show_tags, no_url, ..
        } = &CONFIG.fzf_opts;

        let _text = match (show_tags, no_url) {
            (false, false) => format!("[{}] {}, {}", self.id, self.metadata, self.URL),
            (true, false) => {
                format!(
                    "[{}] {}, {}, {}",
                    self.id,
                    Tags::change_tag_string_delimiter(&(self.tags), " | "),
                    self.metadata,
                    self.URL
                )
            }
            (false, true) => format!("[{}] {}", self.id, self.metadata),
            (true, true) => {
                format!(
                    "[{}] {}, {}",
                    self.id,
                    Tags::change_tag_string_delimiter(&(self.tags), " | "),
                    self.metadata
                )
            }
        };
        Cow::Owned(_text)
    }

    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString<'a> {
        let FzfEnvOpts {
            show_tags, no_url, ..
        } = &CONFIG.fzf_opts;

        // Starting index for tags
        let start_idx_tags = self.id.to_string().len() + 2;

        // Calculate end index for tags if they're shown
        let end_idx_tags = match show_tags {
            false => 0,
            true => {
                let tags = Tags::change_tag_string_delimiter(&(self.tags), " | ");
                start_idx_tags + tags.len() + 1
            }
        };
        let attr_tags = Attr {
            fg: Color::LIGHT_MAGENTA,
            ..Attr::default()
        };

        // Calculate starting index for metadata based on whether tags are shown
        let start_idx_metadata = match show_tags {
            false => self.id.to_string().len() + 2,
            true => end_idx_tags + 1,
        };

        // End index of metadata
        let end_idx_metadata = start_idx_metadata + self.metadata.len();
        let attr_metadata = Attr {
            fg: Color::GREEN,
            // bg: Color::Rgb(5, 10, 15),
            ..Attr::default()
        };

        // Only calculate URL indices if we're showing the URL
        let mut attr_segments = vec![];

        // Always add the tag attributes if tags are shown
        if *show_tags {
            attr_segments.push((attr_tags, (start_idx_tags as u32, end_idx_tags as u32)));
        }

        // Always add the metadata attributes
        attr_segments.push((
            attr_metadata,
            (start_idx_metadata as u32, end_idx_metadata as u32),
        ));

        // Only add URL attributes if URLs are being shown
        if !no_url {
            let start_idx_url = end_idx_metadata + 2; // +2 for ", "
            let end_idx_url = start_idx_url + self.URL.len();
            let attr_url = Attr {
                fg: Color::YELLOW,
                ..Attr::default()
            };
            attr_segments.push((attr_url, (start_idx_url as u32, end_idx_url as u32)));
        }

        AnsiString::new_str(context.text, attr_segments)
    }

    // The preview method can remain unchanged
    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        let text = format!("[{}] {}, {}", &self.id, &self.metadata, &self.URL);
        ItemPreview::AnsiText(format!("\x1b[31mhello:\x1b[m\n{}", text))
    }
}

pub fn fzf_process(bms: &Vec<Bookmark>) {
    let FzfEnvOpts {
        reverse, height, ..
    } = &CONFIG.fzf_opts;

    let options = SkimOptionsBuilder::default()
        .reverse(reverse.to_owned())
        .height(height.to_string())
        .multi(true)
        // Turn on actual filtering so unmatched items are dropped:
        .filter(Some("".to_string()))
        .ansi(true)
        // For full list of accepted keywords see `parse_event` in `src/event.rs`.
        .bind(vec![
            "ctrl-o:accept".to_string(),
            "ctrl-e:accept".to_string(),
            "ctrl-d:accept".to_string(),
            "ctrl-y:accept".to_string(),
        ])
        .build()
        .unwrap();

    // send bookmarks to skim
    let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
    for bm in bms {
        tx_item.send(Arc::new(bm.clone())).unwrap(); // todo check clone
    }
    drop(tx_item); // so that skim could know when to stop waiting for more items.

    let mut stdout = std::io::stdout();
    Skim::run_with(&options, Some(rx_item)).map(|out| match out.final_key {
        Key::Ctrl('e') => {
            let filtered = filter_bms(out);
            // id selection not necessary since all bms are filtered, just open all bms
            let ids = (1..=filtered.len()).map(|i| i as i32).collect();
            debug!("{:?}, {:?}", ids, filtered);
            edit_bms(ids, filtered).unwrap_or_else(|e| {
                debug!("{}", e);
            });
            // clear screen
            // let mut stdout = std::io::stdout();
            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
        }
        Key::Ctrl('o') | Key::Ctrl('y') => {
            let filtered = filter_bms(out);
            // id selection not necessary since all bms are filtered, just open all bms
            let ids: Vec<i32> = (1..=filtered.len()).map(|i| i as i32).collect();
            debug!("{:?}, {:?}", ids, filtered);
            // Change this part to copy the bookmark URLs to the clipboard using the arboard crate
            let mut clipboard = Clipboard::new().unwrap();
            // TODO: do_touch required here
            let urls = filtered.iter().map(|bm| &bm.URL).join("\n");
            clipboard.set_text(urls).unwrap_or_else(|e| {
                debug!("{}", e);
            });
            println!("Copied URLs to clipboard");
            // let mut stdout = std::io::stdout();
            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
        }
        Key::Ctrl('d') => {
            let filtered = filter_bms(out);
            // id selection not necessary since all bms are filtered, just open all bms
            let ids: Vec<i32> = (1..=filtered.len()).map(|i| i as i32).collect();
            debug!("{:?}, {:?}", ids, filtered);
            // Delete the bookmarks
            delete_bms(ids.clone(), filtered.clone()).unwrap_or_else(|e| {
                debug!("{}", e);
            });
            println!("Deleted Bookmark: {:?}", filtered[0].URL);
            // let mut stdout = std::io::stdout();
            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
        }
        Key::Enter => {
            let filtered = filter_bms(out);
            // id selection not necessary since all bms are filtered, just open all bms
            let ids: Vec<i32> = (1..=filtered.len()).map(|i| i as i32).collect();
            debug!("{:?}, {:?}", ids, filtered);
            open_bms(ids, filtered).unwrap_or_else(|e| {
                debug!("{}", e);
            });
            // let mut stdout = std::io::stdout();
            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
        }
        Key::ESC => {
            debug!("Esc");
            // let mut stdout = std::io::stdout();
            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
        }
        _ => (),
    });
}

fn filter_bms(out: SkimOutput) -> Vec<Bookmark> {
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
                .downcast_ref::<Bookmark>()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<Bookmark>>();
    debug!("selected_bms: {:?}", selected_bms);
    selected_bms
}
