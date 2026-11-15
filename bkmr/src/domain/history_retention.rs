// src/domain/history_retention.rs
use crate::domain::per_type::PerType;

/// Saved states kept per bookmark, by type: the `[history.retention]` config section.
///
/// The history triggers save every change and delete; this decides how many of those states
/// survive, counted per bookmark, newest first. It applies whenever bkmr changes or deletes a
/// bookmark, and at startup to every bookmark's history, deleted bookmarks included, so a lowered
/// limit takes effect on the next run. A change of type keeps the larger of both limits until the
/// next change, so it stays undoable. 0 keeps none: then a change or a plain delete cannot be
/// undone. This supersedes "no config
/// can switch it off" in the migration that added history.
pub type HistoryRetention = PerType<u32>;

const DEFAULT_KEEP: u32 = 10;

impl Default for HistoryRetention {
    fn default() -> Self {
        Self::all(DEFAULT_KEEP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use std::collections::HashSet;

    #[test]
    fn given_default_retention_when_asking_any_type_then_keeps_ten() {
        let retention = HistoryRetention::default();

        for system_tag in [
            "topic",
            "_snip_",
            "_imported_",
            "_shell_",
            "_md_",
            "_env_",
            "_mem_",
        ] {
            let tags: HashSet<Tag> = [Tag::new(system_tag).unwrap()].into_iter().collect();
            assert_eq!(retention.for_tags(&tags), 10, "{system_tag}");
        }
    }
}
