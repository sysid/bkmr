// src/domain/embedding_policy.rs
use crate::domain::per_type::PerType;
use crate::domain::tag::Tag;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// How a bookmark type takes part in embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingMode {
    /// Embedded unless explicitly requested otherwise
    On,
    /// Not embedded unless explicitly requested
    Off,
    /// Embedded regardless of any request
    Always,
}

impl std::fmt::Display for EmbeddingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Always => "always",
        })
    }
}

/// Embedding mode per bookmark type: the `[embeddings.policy]` config section.
///
/// The policy decides the value of the `embeddable` flag whenever a bookmark is written; the
/// flag remains the only per-bookmark truth. Changing the policy therefore does not touch
/// existing bookmarks until they are written again.
pub type EmbeddingPolicy = PerType<EmbeddingMode>;

impl Default for EmbeddingPolicy {
    fn default() -> Self {
        Self {
            // Agent memory is retrieved through semantic search
            memory: EmbeddingMode::Always,
            ..Self::all(EmbeddingMode::On)
        }
    }
}

impl EmbeddingPolicy {
    /// Decide the `embeddable` flag. Takes the final tag set rather than the `--type` argument,
    /// because a system tag can also arrive as a plain tag, or from a template, JSON or
    /// frontmatter.
    ///
    /// Precedence: `always` > explicit request > mode default.
    pub fn resolve(&self, tags: &HashSet<Tag>, requested: Option<bool>) -> bool {
        match (self.for_tags(tags), requested) {
            (EmbeddingMode::Always, _) => true,
            (_, Some(explicit)) => explicit,
            (EmbeddingMode::On, None) => true,
            (EmbeddingMode::Off, None) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> HashSet<Tag> {
        values.iter().map(|v| Tag::new(v).unwrap()).collect()
    }

    fn policy_with_each_mode() -> EmbeddingPolicy {
        EmbeddingPolicy {
            uri: EmbeddingMode::On,
            snippet: EmbeddingMode::Off,
            memory: EmbeddingMode::Always,
            ..EmbeddingPolicy::default()
        }
    }

    #[test]
    fn given_mode_and_request_when_resolving_then_applies_precedence() {
        let policy = policy_with_each_mode();
        let on = tags(&["topic"]);
        let off = tags(&["_snip_", "topic"]);
        let always = tags(&["_mem_", "topic"]);

        // (tags, requested, expected): always > explicit request > mode default
        let cases = [
            (&on, None, true),
            (&on, Some(true), true),
            (&on, Some(false), false),
            (&off, None, false),
            (&off, Some(true), true),
            (&off, Some(false), false),
            (&always, None, true),
            (&always, Some(true), true),
            (&always, Some(false), true),
        ];

        for (tags, requested, expected) in cases {
            assert_eq!(
                policy.resolve(tags, requested),
                expected,
                "tags={tags:?} requested={requested:?}"
            );
        }
    }

    #[test]
    fn given_default_policy_when_resolving_then_only_memory_is_forced() {
        let policy = EmbeddingPolicy::default();

        assert!(policy.resolve(&tags(&["_mem_"]), Some(false)));
        for system_tag in ["_snip_", "_shell_", "_md_", "_env_", "_imported_"] {
            assert!(policy.resolve(&tags(&[system_tag]), None), "{system_tag}");
            assert!(
                !policy.resolve(&tags(&[system_tag]), Some(false)),
                "{system_tag}"
            );
        }
        assert!(policy.resolve(&tags(&["topic"]), None));
    }

    #[test]
    fn given_unknown_system_tag_when_resolving_then_follows_uri_mode() {
        let policy = EmbeddingPolicy {
            uri: EmbeddingMode::Off,
            ..EmbeddingPolicy::default()
        };

        assert!(!policy.resolve(&tags(&["_custom_"]), None));
        assert!(!policy.resolve(&tags(&[]), None));
    }

    #[test]
    fn given_each_system_tag_when_resolving_then_uses_its_own_mode() {
        let all_off_except = |tag: &str| {
            let mut policy = EmbeddingPolicy {
                uri: EmbeddingMode::Off,
                snippet: EmbeddingMode::Off,
                text: EmbeddingMode::Off,
                shell: EmbeddingMode::Off,
                markdown: EmbeddingMode::Off,
                env: EmbeddingMode::Off,
                memory: EmbeddingMode::Off,
            };
            match tag {
                "_snip_" => policy.snippet = EmbeddingMode::On,
                "_imported_" => policy.text = EmbeddingMode::On,
                "_shell_" => policy.shell = EmbeddingMode::On,
                "_md_" => policy.markdown = EmbeddingMode::On,
                "_env_" => policy.env = EmbeddingMode::On,
                "_mem_" => policy.memory = EmbeddingMode::On,
                _ => unreachable!("not a known system tag"),
            }
            policy
        };

        for tag in ["_snip_", "_imported_", "_shell_", "_md_", "_env_", "_mem_"] {
            assert!(all_off_except(tag).resolve(&tags(&[tag]), None), "{tag}");
            assert!(
                !all_off_except(tag).resolve(&tags(&["topic"]), None),
                "{tag}"
            );
        }
    }
}
