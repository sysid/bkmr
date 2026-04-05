// src/cli/fzf.rs

use std::borrow::Cow;
use std::io::Write;

use crate::cli::bookmark_commands;
use crate::cli::error::CliResult;
use crate::cli::process::{
    clone_bookmark, copy_bookmark_url_to_clipboard, copy_url_to_clipboard, delete_bookmarks,
    edit_bookmarks, execute_bookmark_default_action,
};
use crate::domain::bookmark::Bookmark;
use crate::domain::search::SemanticSearchResult;
use crate::domain::system_tag::SystemTag;
use crate::infrastructure::di::ServiceContainer;
use crate::util::helper::{format_file_path, format_mtime};
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::Stylize;
use crossterm::{
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use skim::{prelude::*, DisplayContext, ItemPreview, PreviewContext, Skim, SkimItem};
use tracing::{debug, instrument};

/// Unified skim item for both classic and enhanced display styles.
/// The difference between styles is in how `display_text` and `preview_text`
/// are constructed at creation time.
#[derive(Clone)]
struct FzfBookmarkItem {
    bookmark: Bookmark,
    display_text: String,
    preview_text: String,
    /// Pre-parsed segments for colored display (start, end, color)
    color_segments: Vec<(usize, usize, Color)>,
}

impl FzfBookmarkItem {
    fn new_classic(
        bookmark: &Bookmark,
        max_id_width: usize,
        action_description: &str,
        settings: &crate::config::Settings,
    ) -> Self {
        let display_text =
            create_bookmark_display_text(bookmark, max_id_width, action_description, settings);
        let preview_text = create_bookmark_preview_text(bookmark, action_description);
        let color_segments =
            compute_color_segments(&display_text, bookmark, max_id_width, &settings.fzf_opts);

        Self {
            bookmark: bookmark.clone(),
            display_text,
            preview_text,
            color_segments,
        }
    }

    fn new_enhanced(
        bookmark: &Bookmark,
        max_id_width: usize,
        action_description: &str,
        services: &ServiceContainer,
        show_action: bool,
    ) -> Self {
        let id = bookmark.id.unwrap_or(0);
        let display_text = format!("{:>width$}: {}", id, bookmark.title, width = max_id_width);
        let preview_text =
            create_enhanced_preview(bookmark, action_description, services, show_action);

        Self {
            bookmark: bookmark.clone(),
            display_text,
            preview_text,
            color_segments: Vec::new(), // Enhanced style uses plain text display
        }
    }
}

impl SkimItem for FzfBookmarkItem {
    fn text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.display_text)
    }

    fn display(&self, context: DisplayContext) -> Line<'_> {
        if self.color_segments.is_empty() {
            // Enhanced style: use default highlighting
            return context.to_line(self.text());
        }

        // Classic style: apply color segments
        let text = &self.display_text;
        let mut spans = Vec::new();
        let mut pos = 0;

        for &(start, end, color) in &self.color_segments {
            let start = start.min(text.len());
            let end = end.min(text.len());
            if start > pos {
                spans.push(Span::raw(&text[pos..start]));
            }
            if start < end {
                spans.push(Span::styled(&text[start..end], Style::default().fg(color)));
            }
            pos = end;
        }
        if pos < text.len() {
            spans.push(Span::raw(&text[pos..]));
        }

        Line::from(spans)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        ItemPreview::AnsiText(self.preview_text.clone())
    }

    fn output(&self) -> Cow<'_, str> {
        Cow::Owned(self.bookmark.id.unwrap_or(0).to_string())
    }
}

impl SkimItem for SemanticSearchResult {
    fn text(&self) -> Cow<'_, str> {
        Cow::Owned(self.display())
    }
}

/// Compute color segments sorted by position for the classic display style
fn compute_color_segments(
    text: &str,
    bookmark: &Bookmark,
    max_id_width: usize,
    fzf_opts: &crate::config::FzfOpts,
) -> Vec<(usize, usize, Color)> {
    let mut segments = Vec::new();
    let padding = max_id_width + 2; // "ID: "
    let title_end = padding + bookmark.title.len();

    // Title in green
    segments.push((padding, title_end, Color::Green));

    // URL in yellow (within angle brackets)
    if !fzf_opts.no_url {
        if let (Some(start), Some(end)) = (text.find('<'), text.find('>')) {
            segments.push((start, end + 1, Color::Yellow));
        }
    }

    // Tags in magenta (within square brackets)
    if fzf_opts.show_tags && !bookmark.tags.is_empty() {
        if let (Some(start), Some(end)) = (text.find('['), text.find(']')) {
            segments.push((start, end + 1, Color::Magenta));
        }
    }

    // Action in cyan (within parentheses)
    if fzf_opts.show_action {
        if let (Some(start), Some(end)) = (text.rfind('('), text.rfind(')')) {
            segments.push((start, end + 1, Color::Cyan));
        }
    }

    // File info in dark grey
    if fzf_opts.show_file_info {
        if let Some(start) = text.find("📁") {
            let end = text[start..]
                .find('\n')
                .map(|p| start + p)
                .unwrap_or(text.len());
            segments.push((start, end, Color::DarkGray));
        }
    }

    // Sort by start position for correct span construction
    segments.sort_by_key(|s| s.0);
    segments
}

/// Create display text for a bookmark with proper formatting
fn create_bookmark_display_text(
    bookmark: &Bookmark,
    max_id_width: usize,
    action_description: &str,
    settings: &crate::config::Settings,
) -> String {
    let id = bookmark.id.unwrap_or(0);
    let title = &bookmark.title;
    let url = &bookmark.url;
    let binding = bookmark.formatted_tags();
    let tags_str = binding.trim_matches(',');

    let fzf_opts = &settings.fzf_opts;

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
            id,
            title,
            action_display,
            tags_display,
            width = max_id_width
        )
    } else {
        format!(
            "{:>width$}: {} <{}>{}{}",
            id,
            title,
            url,
            action_display,
            tags_display,
            width = max_id_width
        )
    };

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

    if let (Some(file_path), Some(file_mtime)) = (&bookmark.file_path, bookmark.file_mtime) {
        let formatted_path = format_file_path(file_path, 120);
        let formatted_time = format_mtime(file_mtime);
        preview_text.push_str(&format!(
            "\n\nSource: {} ({})",
            formatted_path, formatted_time
        ));
    }

    format!("{}\n{}", "Bookmark Details:".bold(), preview_text)
}

/// Create enhanced preview text with ANSI colors
fn create_enhanced_preview(
    bookmark: &Bookmark,
    action_description: &str,
    services: &ServiceContainer,
    show_action: bool,
) -> String {
    // Render template variables in URL if present
    let rendered_url = if bookmark.url.contains("{{") || bookmark.url.contains("{%") {
        services
            .interpolation_service
            .render_bookmark_url(bookmark)
            .unwrap_or_else(|_| bookmark.url.clone())
    } else {
        bookmark.url.clone()
    };

    let tags_str = bookmark
        .formatted_tags()
        .replace(',', " ")
        .trim()
        .to_string();

    let mut preview = format!(
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
        rendered_url,
    );

    if show_action {
        preview.push_str(&format!(
            "\n\n{}: {}",
            "Default Action".magenta().bold(),
            action_description
        ));
    }

    if !tags_str.is_empty() {
        preview.push_str(&format!("\n\n{}: {}", "Tags".blue().bold(), tags_str));
    }

    if let (Some(file_path), Some(file_mtime)) = (&bookmark.file_path, &bookmark.file_mtime) {
        let formatted_path = format_file_path(file_path, 120);
        let formatted_time = format_mtime(*file_mtime);
        preview.push_str(&format!(
            "\n\n{}: {} ({})",
            "Source File".dark_grey().bold(),
            formatted_path,
            formatted_time
        ));
    }

    preview
}

/// Extract selected bookmarks from skim output
fn get_selected_bookmarks(output: &SkimOutput, bookmarks: &[Bookmark]) -> Vec<Bookmark> {
    let selected_ids: Vec<i32> = output
        .selected_items
        .iter()
        .filter_map(|matched| {
            let id_str = matched.item.output();
            id_str.parse::<i32>().ok()
        })
        .collect();

    let selected: Vec<Bookmark> = bookmarks
        .iter()
        .filter(|b| b.id.is_some() && selected_ids.contains(&b.id.unwrap()))
        .cloned()
        .collect();

    if !selected.is_empty() {
        eprintln!("Selected bookmarks:");
        for bookmark in &selected {
            eprintln!(" - {}: {}", bookmark.id.unwrap_or(0), bookmark.title);
        }
    }

    debug!("Selected {} bookmarks", selected.len());
    selected
}

/// Processes bookmarks using the fzf-like selector interface
#[instrument(skip(bookmarks), level = "debug")]
pub fn fzf_process(
    bookmarks: &[Bookmark],
    style: &str,
    services: &ServiceContainer,
    settings: &crate::config::Settings,
    stdout: bool,
) -> CliResult<()> {
    if bookmarks.is_empty() {
        eprintln!("No bookmarks to display");
        return Ok(());
    }

    // Bookmarks arrive pre-sorted by the query layer
    let max_id_width = bookmarks
        .iter()
        .map(|b| b.id.unwrap_or(0).to_string().len())
        .max()
        .unwrap_or(0);

    let fzf_opts = &settings.fzf_opts;

    // Build skim options
    let mut options = SkimOptions::default();
    options.height = fzf_opts.height.clone();
    options.reverse = fzf_opts.reverse;
    options.multi = false;
    options.ansi = true;
    options.query = Some("".to_string());

    if style == "enhanced" {
        options.preview = Some("".to_string());
        options.preview_window = "right:70%:wrap".into();
    }

    options.bind = vec![
        "ctrl-a:accept".to_string(),
        "ctrl-o:accept".to_string(),
        "ctrl-y:accept".to_string(),
        "ctrl-e:accept".to_string(),
        "ctrl-d:accept".to_string(),
        "ctrl-p:accept".to_string(),
        "enter:accept".to_string(),
        "esc:abort".to_string(),
    ];

    // Finalize options (processes bind strings into keymap, applies defaults)
    let options = options.build();

    // Build items based on style
    let items: Vec<FzfBookmarkItem> = bookmarks
        .iter()
        .map(|bookmark| {
            let base_description = services
                .action_service
                .get_default_action_description(bookmark);
            let action_description = bookmark_commands::format_action_description(
                base_description,
                bookmark.opener.as_ref(),
            );

            if style == "enhanced" {
                FzfBookmarkItem::new_enhanced(
                    bookmark,
                    max_id_width,
                    &action_description,
                    services,
                    fzf_opts.show_action,
                )
            } else {
                FzfBookmarkItem::new_classic(
                    bookmark,
                    max_id_width,
                    &action_description,
                    settings,
                )
            }
        })
        .collect();

    // Alternate screen for non-fullscreen, non-stdout mode
    let use_alternate_screen = !stdout && fzf_opts.height != "100%" && fzf_opts.height != "100";
    if use_alternate_screen {
        execute!(std::io::stdout(), EnterAlternateScreen)?;
    }

    let skim_output = Skim::run_items(options, items).map_err(|e| {
        crate::cli::error::CliError::CommandFailed(format!("Skim failed: {}", e))
    });

    if use_alternate_screen {
        execute!(std::io::stdout(), LeaveAlternateScreen)?;
    }

    let output = match skim_output {
        Ok(output) => output,
        Err(e) => {
            reset_terminal_state(stdout);
            return Err(e);
        }
    };

    debug!("Final key: {:?}", output.final_key);

    if output.is_abort {
        debug!("Selection aborted");
        reset_terminal_state(stdout);
        return Ok(());
    }

    let selected_bookmarks = get_selected_bookmarks(&output, bookmarks);

    if selected_bookmarks.is_empty() {
        debug!("No bookmarks selected");
        return Ok(());
    }

    let ids: Vec<i32> = selected_bookmarks.iter().filter_map(|bm| bm.id).collect();
    debug!("Selected bookmark IDs: {:?}", ids);

    println!();

    let key = output.final_key;
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => {
            if stdout {
                for bookmark in &selected_bookmarks {
                    let content = services
                        .interpolation_service
                        .render_bookmark_url(bookmark)
                        .map_err(|e| {
                            crate::cli::error::CliError::CommandFailed(format!(
                                "Failed to render content: {}",
                                e
                            ))
                        })?;
                    println!("{}", content);
                }
            } else {
                for bookmark in &selected_bookmarks {
                    execute_bookmark_default_action(bookmark, services.action_service.clone())?;
                }
            }
        }
        (KeyCode::Char('y'), m) | (KeyCode::Char('o'), m) if m.contains(KeyModifiers::CONTROL) => {
            if let Some(bookmark) = selected_bookmarks.first() {
                let is_shell_script = bookmark
                    .tags
                    .iter()
                    .any(|tag| tag.is_system_tag_of(SystemTag::Shell));

                if is_shell_script {
                    let command = format!("bkmr open --no-edit {} --", bookmark.id.unwrap_or(0));
                    copy_url_to_clipboard(&command, services.clipboard_service.clone())?;
                } else {
                    copy_bookmark_url_to_clipboard(
                        bookmark,
                        services.interpolation_service.clone(),
                        services.clipboard_service.clone(),
                    )?;
                }
            }
        }
        (KeyCode::Char('e'), m) if m.contains(KeyModifiers::CONTROL) => {
            edit_bookmarks(
                ids,
                false,
                services.bookmark_service.clone(),
                services.template_service.clone(),
                settings,
            )?;
        }
        (KeyCode::Char('d'), m) if m.contains(KeyModifiers::CONTROL) => {
            delete_bookmarks(ids, services.bookmark_service.clone(), settings)?;
        }
        (KeyCode::Char('a'), m) if m.contains(KeyModifiers::CONTROL) => {
            if let Some(bookmark) = selected_bookmarks.first() {
                if let Some(id) = bookmark.id {
                    clone_bookmark(
                        id,
                        services.bookmark_service.clone(),
                        services.template_service.clone(),
                    )?;
                }
            }
        }
        (KeyCode::Char('p'), m) if m.contains(KeyModifiers::CONTROL) => {
            if let Some(bookmark) = selected_bookmarks.first() {
                let _ = execute!(std::io::stdout(), Clear(ClearType::FromCursorDown));
                let details = bookmark_commands::show_bookmark_details(bookmark, services);
                print!("{}", details);
            }
        }
        _ => {
            debug!("Unhandled key: {:?}", key);
        }
    }

    reset_terminal_state(stdout);

    Ok(())
}

/// Reset terminal state after skim exit.
/// Skipped in --stdout mode to avoid polluting captured output.
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
