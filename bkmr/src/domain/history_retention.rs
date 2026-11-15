// src/domain/history_retention.rs
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Saved states kept per bookmark, by type: the `[history.retention]` config section.
///
/// The history triggers save every change and delete; this decides how many of those states
/// survive, counted per bookmark, newest first. It applies whenever bkmr changes or deletes a
/// bookmark, so existing states beyond the limit go on that bookmark's next write, never all at
/// once. 0 keeps none: then a change or a plain delete cannot be undone. This supersedes "no config
/// can switch it off" in the migration that added history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryRetention {
    /// Bookmarks without a known system tag
    pub uri: u32,
    #[serde(rename = "_snip_")]
    pub snippet: u32,
    #[serde(rename = "_imported_")]
    pub text: u32,
    #[serde(rename = "_shell_")]
    pub shell: u32,
    #[serde(rename = "_md_")]
    pub markdown: u32,
    #[serde(rename = "_env_")]
    pub env: u32,
    #[serde(rename = "_mem_")]
    pub memory: u32,
}

const DEFAULT_KEEP: u32 = 10;

impl Default for HistoryRetention {
    fn default() -> Self {
        Self {
            uri: DEFAULT_KEEP,
            snippet: DEFAULT_KEEP,
            text: DEFAULT_KEEP,
            shell: DEFAULT_KEEP,
            markdown: DEFAULT_KEEP,
            env: DEFAULT_KEEP,
            memory: DEFAULT_KEEP,
        }
    }
}

impl HistoryRetention {
    /// How many saved states a bookmark with these tags keeps
    pub fn keep_for(&self, tags: &HashSet<Tag>) -> u32 {
        match SystemTag::of_bookmark(tags) {
            SystemTag::Snippet => self.snippet,
            SystemTag::Text => self.text,
            SystemTag::Shell => self.shell,
            SystemTag::Markdown => self.markdown,
            SystemTag::Env => self.env,
            SystemTag::Memory => self.memory,
            SystemTag::Uri => self.uri,
        }
    }

    /// Config key and limit of each bookmark type, for display.
    pub fn entries(&self) -> [(&'static str, u32); 7] {
        [
            ("uri", self.uri),
            (SystemTag::Snippet.as_str(), self.snippet),
            (SystemTag::Text.as_str(), self.text),
            (SystemTag::Shell.as_str(), self.shell),
            (SystemTag::Markdown.as_str(), self.markdown),
            (SystemTag::Env.as_str(), self.env),
            (SystemTag::Memory.as_str(), self.memory),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> HashSet<Tag> {
        values.iter().map(|v| Tag::new(v).unwrap()).collect()
    }

    #[test]
    fn given_default_retention_when_asking_any_type_then_keeps_ten() {
        let retention = HistoryRetention::default();

        for type_tags in [
            vec!["topic"],
            vec!["_snip_"],
            vec!["_imported_"],
            vec!["_shell_"],
            vec!["_md_"],
            vec!["_env_"],
            vec!["_mem_"],
        ] {
            assert_eq!(retention.keep_for(&tags(&type_tags)), 10, "{type_tags:?}");
        }
    }

    #[test]
    fn given_limit_per_type_when_asking_then_uses_the_bookmark_type() {
        let retention = HistoryRetention {
            uri: 1,
            snippet: 2,
            text: 3,
            shell: 4,
            markdown: 5,
            env: 6,
            memory: 7,
        };

        assert_eq!(retention.keep_for(&tags(&["topic"])), 1);
        assert_eq!(retention.keep_for(&tags(&["_snip_", "rust"])), 2);
        assert_eq!(retention.keep_for(&tags(&["_imported_"])), 3);
        assert_eq!(retention.keep_for(&tags(&["_shell_"])), 4);
        assert_eq!(retention.keep_for(&tags(&["_md_"])), 5);
        assert_eq!(retention.keep_for(&tags(&["_env_"])), 6);
        assert_eq!(retention.keep_for(&tags(&["_mem_", "fact"])), 7);
    }

    #[test]
    fn given_unknown_system_tag_when_asking_then_follows_uri_limit() {
        let retention = HistoryRetention {
            uri: 4,
            ..HistoryRetention::default()
        };

        assert_eq!(retention.keep_for(&tags(&["_custom_"])), 4);
    }
}
