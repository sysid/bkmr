// src/cli/history_diff.rs
//! Unified diffs between states of an agent memory, for `history -p` and `restore --dry-run`.

use crate::domain::bookmark::Bookmark;
use crate::domain::history::HistoryEntry;
use crate::domain::tag::Tag;
use crossterm::style::Stylize;
use similar::TextDiff;
use std::collections::HashSet;

/// Tags as `a,b`: sorted, without the stored format's outer commas
fn display_tags(tags: &HashSet<Tag>) -> String {
    Tag::format_tags(tags).trim_matches(',').to_string()
}

/// Tags of a history row in the stored format `,b,a,` as `a,b`
fn display_stored_tags(stored: &str) -> String {
    match Tag::parse_tags(stored) {
        Ok(tags) => display_tags(&tags),
        // Only `history -p` reads rows this way, and one odd row should not hide the others.
        // `restore --dry-run` goes through the service, which fails on it like the restore.
        Err(_) => stored.trim_matches(',').to_string(),
    }
}

/// The fields of a memory that history keeps and restore brings back
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryState {
    pub title: String,
    /// Sorted and comma-separated, without the stored format's outer commas
    pub tags: String,
    pub description: String,
    pub url: String,
}

impl MemoryState {
    pub fn from_entry(entry: &HistoryEntry) -> Self {
        Self {
            title: entry.title.clone(),
            tags: display_stored_tags(&entry.tags),
            description: entry.description.clone(),
            url: entry.url.clone(),
        }
    }

    pub fn from_bookmark(bookmark: &Bookmark) -> Self {
        Self {
            title: bookmark.title.clone(),
            tags: display_tags(&bookmark.tags),
            description: bookmark.description.clone(),
            url: bookmark.url.clone(),
        }
    }

    /// One document per state, so a single line diff covers every field: metadata as
    /// `key: value` lines, then the content after a blank line
    fn to_text(&self) -> String {
        format!(
            "title: {}\ntags: {}\ndescription: {}\n\n{}",
            self.title, self.tags, self.description, self.url
        )
    }
}

/// A unified diff from `old` to `new`, where `None` is a memory that does not exist. Empty if
/// nothing differs.
pub fn render_patch(
    old: Option<&MemoryState>,
    new: Option<&MemoryState>,
    old_label: &str,
    new_label: &str,
    color: bool,
) -> String {
    let mut old_text = old.map(MemoryState::to_text).unwrap_or_default();
    let mut new_text = new.map(MemoryState::to_text).unwrap_or_default();
    if old_text == new_text {
        return String::new();
    }
    // Most memories end without a newline, and the "No newline at end of file" marker would
    // then follow nearly every changed last line. With neither side ending in one, both get
    // it; when only one side does, the newline is part of the change and the marker shows it.
    let lacks_newline = |text: &str| !text.ends_with('\n');
    if lacks_newline(&old_text) && lacks_newline(&new_text) {
        for text in [&mut old_text, &mut new_text] {
            if !text.is_empty() {
                text.push('\n');
            }
        }
    }

    let diff = TextDiff::from_lines(&old_text, &new_text);
    let patch = diff
        .unified_diff()
        .context_radius(3)
        .header(old_label, new_label)
        .to_string();
    if color {
        colorize(&patch)
    } else {
        patch
    }
}

/// git's colors: bold file headers, cyan hunk headers, red removals, green additions
fn colorize(patch: &str) -> String {
    let mut in_header = true;
    patch
        // Keeps each line's ending, `\r` included, so color changes nothing but the color
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\r', '\n']);
            let ending = &line[body.len()..];
            // A content line can start with `--- ` too; only lines before the first hunk are headers
            if body.starts_with("@@") {
                in_header = false;
            }
            let styled = if in_header {
                body.bold().to_string()
            } else if body.starts_with("@@") {
                body.cyan().to_string()
            } else if body.starts_with('-') {
                body.red().to_string()
            } else if body.starts_with('+') {
                body.green().to_string()
            } else {
                body.to_string()
            };
            styled + ending
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(title: &str, tags: &str, url: &str) -> MemoryState {
        MemoryState {
            title: title.to_string(),
            tags: tags.to_string(),
            description: String::new(),
            url: url.to_string(),
        }
    }

    #[test]
    fn given_history_entry_with_stored_tags_when_converting_then_tags_are_sorted_without_outer_commas(
    ) {
        let entry = HistoryEntry {
            history_id: 1,
            bookmark_id: 42,
            change: crate::domain::history::HistoryChange::Update,
            url: "text".to_string(),
            title: "Title".to_string(),
            description: "desc".to_string(),
            tags: ",fact,_mem_,".to_string(),
            changed_at: chrono::Utc::now(),
            embeddable: true,
            file_path: None,
            file_mtime: None,
            file_hash: None,
            opener: None,
        };

        let state = MemoryState::from_entry(&entry);

        assert_eq!(state.tags, "_mem_,fact");
        assert_eq!(state.url, "text");
        assert_eq!(state.title, "Title");
        assert_eq!(state.description, "desc");
    }

    #[test]
    fn given_changed_content_line_when_rendering_then_unified_hunk_shows_old_and_new_line() {
        let old = state(
            "Auth",
            "_mem_,fact",
            "JWT expiry is 24h.\nRefresh tokens in Redis.",
        );
        let new = state(
            "Auth",
            "_mem_,fact",
            "JWT expiry is 12h.\nRefresh tokens in Redis.",
        );

        let patch = render_patch(Some(&old), Some(&new), "history 3", "#42 current", false);

        assert!(
            patch.starts_with("--- history 3\n+++ #42 current\n@@ "),
            "{patch}"
        );
        assert!(
            patch.contains("\n-JWT expiry is 24h.\n+JWT expiry is 12h.\n"),
            "{patch}"
        );
        assert!(patch.contains("\n Refresh tokens in Redis.\n"), "{patch}");
    }

    #[test]
    fn given_only_title_changed_when_rendering_then_content_lines_are_not_marked_changed() {
        let old = state("Old title", "_mem_,fact", "unchanged text");
        let new = state("New title", "_mem_,fact", "unchanged text");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", false);

        assert!(
            patch.contains("\n-title: Old title\n+title: New title\n"),
            "{patch}"
        );
        assert!(!patch.contains("-unchanged text"), "{patch}");
        assert!(!patch.contains("+unchanged text"), "{patch}");
    }

    #[test]
    fn given_no_new_state_when_rendering_then_every_line_is_removed() {
        let old = state("Gone", "_mem_,fact", "deleted text");

        let patch = render_patch(Some(&old), None, "history 7", "/dev/null", false);

        assert!(patch.contains("+++ /dev/null\n"), "{patch}");
        assert!(patch.contains("\n-title: Gone\n"), "{patch}");
        assert!(patch.contains("\n-deleted text\n"), "{patch}");
        let added_lines = patch
            .lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++ "))
            .count();
        assert_eq!(added_lines, 0, "{patch}");
    }

    #[test]
    fn given_no_old_state_when_rendering_then_every_line_is_added() {
        let new = state("Back", "_mem_,fact", "restored text");

        let patch = render_patch(None, Some(&new), "/dev/null", "history 7", false);

        assert!(patch.starts_with("--- /dev/null\n"), "{patch}");
        assert!(patch.contains("\n+restored text\n"), "{patch}");
    }

    #[test]
    fn given_equal_states_when_rendering_then_patch_is_empty() {
        let same = state("Same", "_mem_,fact", "text");

        assert_eq!(render_patch(Some(&same), Some(&same), "a", "b", false), "");
    }

    #[test]
    fn given_content_without_trailing_newline_when_rendering_then_no_missing_newline_marker() {
        let old = state("T", "_mem_", "one");
        let new = state("T", "_mem_", "two");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", false);

        assert!(!patch.contains("No newline at end of file"), "{patch}");
    }

    #[test]
    fn given_only_a_trailing_newline_added_when_rendering_then_the_change_is_shown() {
        let old = state("T", "_mem_", "JWT 24h");
        let new = state("T", "_mem_", "JWT 24h\n");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", false);

        assert!(
            patch.contains("-JWT 24h\n\\ No newline at end of file\n+JWT 24h\n"),
            "{patch}"
        );
    }

    #[test]
    fn given_content_lines_that_look_like_file_headers_when_coloring_then_they_are_colored_as_changes(
    ) {
        let old = state("T", "_mem_", "-- old note");
        let new = state("T", "_mem_", "++ new note");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", true);

        assert!(
            patch.contains(&"--- old note".red().to_string()),
            "{patch:?}"
        );
        assert!(
            patch.contains(&"+++ new note".green().to_string()),
            "{patch:?}"
        );
        assert!(patch.contains(&"--- a".bold().to_string()), "{patch:?}");
    }

    #[test]
    fn given_crlf_content_when_coloring_then_carriage_returns_are_kept() {
        let old = state("T", "_mem_", "one\r\ntwo\r\n");
        let new = state("T", "_mem_", "one\r\nthree\r\n");

        let plain = render_patch(Some(&old), Some(&new), "a", "b", false);
        let colored = render_patch(Some(&old), Some(&new), "a", "b", true);

        assert_eq!(
            colored.matches('\r').count(),
            plain.matches('\r').count(),
            "{colored:?}"
        );
        assert!(plain.contains('\r'), "{plain:?}");
    }

    #[test]
    fn given_color_off_when_rendering_then_no_escape_sequences() {
        let old = state("T", "_mem_", "one");
        let new = state("T", "_mem_", "two");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", false);

        assert!(!patch.contains('\x1b'), "{patch:?}");
    }

    #[test]
    fn given_color_on_when_rendering_then_removed_and_added_lines_are_colored() {
        let old = state("T", "_mem_", "one");
        let new = state("T", "_mem_", "two");

        let patch = render_patch(Some(&old), Some(&new), "a", "b", true);

        assert!(patch.contains('\x1b'), "{patch:?}");
        assert!(patch.contains("-one"), "{patch:?}");
        assert!(patch.contains("+two"), "{patch:?}");
    }
}
