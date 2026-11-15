// src/domain/per_type.rs
use crate::domain::system_tag::SystemTag;
use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One setting per bookmark type, keyed by system tag in config (`uri`, `_snip_`, ...).
///
/// Shared by the config sections that vary by type (`[embeddings.policy]`,
/// `[history.retention]`), so a new system tag is added in one place. Each section supplies its
/// own `Default`; keys left out of the config keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "T: Deserialize<'de>, PerType<T>: Default")
)]
pub struct PerType<T> {
    /// Bookmarks without a known system tag
    pub uri: T,
    #[serde(rename = "_snip_")]
    pub snippet: T,
    #[serde(rename = "_imported_")]
    pub text: T,
    #[serde(rename = "_shell_")]
    pub shell: T,
    #[serde(rename = "_md_")]
    pub markdown: T,
    #[serde(rename = "_env_")]
    pub env: T,
    #[serde(rename = "_mem_")]
    pub memory: T,
}

impl<T: Copy> PerType<T> {
    /// The same value for every type
    pub const fn all(value: T) -> Self {
        Self {
            uri: value,
            snippet: value,
            text: value,
            shell: value,
            markdown: value,
            env: value,
            memory: value,
        }
    }

    /// A value computed for each type
    pub fn from_fn(value_for: impl Fn(SystemTag) -> T) -> Self {
        Self {
            uri: value_for(SystemTag::Uri),
            snippet: value_for(SystemTag::Snippet),
            text: value_for(SystemTag::Text),
            shell: value_for(SystemTag::Shell),
            markdown: value_for(SystemTag::Markdown),
            env: value_for(SystemTag::Env),
            memory: value_for(SystemTag::Memory),
        }
    }

    /// The value for a bookmark of this type
    pub fn for_type(&self, bookmark_type: SystemTag) -> T {
        match bookmark_type {
            SystemTag::Snippet => self.snippet,
            SystemTag::Text => self.text,
            SystemTag::Shell => self.shell,
            SystemTag::Markdown => self.markdown,
            SystemTag::Env => self.env,
            SystemTag::Memory => self.memory,
            SystemTag::Uri => self.uri,
        }
    }

    /// The value for a bookmark with these tags
    pub fn for_tags(&self, tags: &HashSet<Tag>) -> T {
        self.for_type(SystemTag::of_bookmark(tags))
    }

    /// Config key and value of each bookmark type, for display.
    pub fn entries(&self) -> [(&'static str, T); 7] {
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

    fn numbered() -> PerType<u32> {
        PerType {
            uri: 1,
            snippet: 2,
            text: 3,
            shell: 4,
            markdown: 5,
            env: 6,
            memory: 7,
        }
    }

    #[test]
    fn given_value_per_type_when_asking_by_tags_then_uses_the_bookmark_type() {
        let per_type = numbered();

        assert_eq!(per_type.for_tags(&tags(&["topic"])), 1);
        assert_eq!(per_type.for_tags(&tags(&["_snip_", "rust"])), 2);
        assert_eq!(per_type.for_tags(&tags(&["_imported_"])), 3);
        assert_eq!(per_type.for_tags(&tags(&["_shell_"])), 4);
        assert_eq!(per_type.for_tags(&tags(&["_md_"])), 5);
        assert_eq!(per_type.for_tags(&tags(&["_env_"])), 6);
        assert_eq!(per_type.for_tags(&tags(&["_mem_", "fact"])), 7);
    }

    #[test]
    fn given_function_of_type_when_building_then_each_type_gets_its_own_value() {
        let per_type = PerType::from_fn(|t| t.as_str().len());

        assert_eq!(per_type.uri, 0);
        assert_eq!(per_type.snippet, "_snip_".len());
        assert_eq!(per_type.text, "_imported_".len());
        assert_eq!(per_type.shell, "_shell_".len());
        assert_eq!(per_type.markdown, "_md_".len());
        assert_eq!(per_type.env, "_env_".len());
        assert_eq!(per_type.memory, "_mem_".len());
    }

    #[test]
    fn given_unknown_system_tag_when_asking_then_follows_uri() {
        assert_eq!(numbered().for_tags(&tags(&["_custom_"])), 1);
        assert_eq!(numbered().for_tags(&tags(&[])), 1);
    }
}
