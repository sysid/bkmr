use crate::cli::error::{CliError, CliResult};
use crate::domain::tag::Tag;
use std::collections::HashSet;
use tracing::{debug, instrument};

/// Utility for processing and parsing common CLI argument patterns
pub struct ArgumentProcessor;

impl ArgumentProcessor {
    /// Parse a tag string into a HashSet of Tag objects
    /// This centralizes the tag parsing logic used across multiple commands
    #[instrument(level = "trace")]
    pub fn parse_tag_string(tag_str: &Option<String>) -> Option<HashSet<Tag>> {
        Tag::parse_tag_option(tag_str.as_ref().map(|s| s.as_str())).unwrap_or_else(|e| {
            debug!("Failed to parse tags: {}", e);
            None
        })
    }

    /// Apply prefix tags to base tags, returning a new set with all tags combined
    /// This is used in search operations where prefix tags modify base tags
    #[instrument(level = "trace")]
    pub fn apply_prefix_tags(
        base_tags: Option<HashSet<Tag>>,
        prefix_tags: Option<HashSet<Tag>>,
    ) -> Option<HashSet<Tag>> {
        match (base_tags, prefix_tags) {
            (None, None) => None,
            (Some(base), None) => Some(base),
            (None, Some(prefix)) => Some(prefix),
            (Some(mut base), Some(prefix)) => {
                base.extend(prefix);
                Some(base)
            }
        }
    }

    /// Parse tags with error handling for CLI commands
    /// Returns a CliResult with proper error formatting
    pub fn parse_tags_with_error_handling(tag_str: &Option<String>) -> CliResult<HashSet<Tag>> {
        match Tag::parse_tag_option(tag_str.as_deref()) {
            Ok(Some(tags)) => Ok(tags),
            Ok(None) => Ok(HashSet::new()),
            Err(e) => Err(CliError::InvalidInput(format!(
                "Failed to parse tags: {}",
                e
            ))),
        }
    }

    /// Process all search-related tag parameters with prefixes
    /// This centralizes the complex tag processing logic from the search command
    #[instrument(level = "trace")]
    pub fn process_search_tag_parameters(
        tags_exact: &Option<String>,
        tags_exact_prefix: &Option<String>,
        tags_all: &Option<String>,
        tags_all_prefix: &Option<String>,
        tags_all_not: &Option<String>,
        tags_all_not_prefix: &Option<String>,
        tags_any: &Option<String>,
        tags_any_prefix: &Option<String>,
        tags_any_not: &Option<String>,
        tags_any_not_prefix: &Option<String>,
    ) -> SearchTagParams {
        SearchTagParams {
            exact_tags: Self::apply_prefix_tags(
                Self::parse_tag_string(tags_exact),
                Self::parse_tag_string(tags_exact_prefix),
            ),
            all_tags: Self::apply_prefix_tags(
                Self::parse_tag_string(tags_all),
                Self::parse_tag_string(tags_all_prefix),
            ),
            all_not_tags: Self::apply_prefix_tags(
                Self::parse_tag_string(tags_all_not),
                Self::parse_tag_string(tags_all_not_prefix),
            ),
            any_tags: Self::apply_prefix_tags(
                Self::parse_tag_string(tags_any),
                Self::parse_tag_string(tags_any_prefix),
            ),
            any_not_tags: Self::apply_prefix_tags(
                Self::parse_tag_string(tags_any_not),
                Self::parse_tag_string(tags_any_not_prefix),
            ),
        }
    }
}

/// Processed search tag parameters ready for use in query building
#[derive(Debug)]
pub struct SearchTagParams {
    pub exact_tags: Option<HashSet<Tag>>,
    pub all_tags: Option<HashSet<Tag>>,
    pub all_not_tags: Option<HashSet<Tag>>,
    pub any_tags: Option<HashSet<Tag>>,
    pub any_not_tags: Option<HashSet<Tag>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_valid_tag_string_when_parse_tag_string_then_returns_tag_set() {
        let tag_str = Some("tag1,tag2,tag3".to_string());
        let result = ArgumentProcessor::parse_tag_string(&tag_str);

        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn given_none_tag_string_when_parse_tag_string_then_returns_none() {
        let result = ArgumentProcessor::parse_tag_string(&None);
        assert!(result.is_none());
    }

    #[test]
    fn given_base_and_prefix_tags_when_apply_prefix_tags_then_returns_merged_set() {
        let base = {
            let mut set = HashSet::new();
            set.insert(Tag::new("base1").unwrap());
            set.insert(Tag::new("base2").unwrap());
            Some(set)
        };

        let prefix = {
            let mut set = HashSet::new();
            set.insert(Tag::new("prefix1").unwrap());
            Some(set)
        };

        let result = ArgumentProcessor::apply_prefix_tags(base, prefix);
        assert!(result.is_some());

        let combined = result.unwrap();
        assert_eq!(combined.len(), 3);
        assert!(combined.contains(&Tag::new("base1").unwrap()));
        assert!(combined.contains(&Tag::new("base2").unwrap()));
        assert!(combined.contains(&Tag::new("prefix1").unwrap()));
    }

    #[test]
    fn given_only_base_tags_when_apply_prefix_tags_then_returns_base_set() {
        let base = {
            let mut set = HashSet::new();
            set.insert(Tag::new("base1").unwrap());
            Some(set)
        };

        let result = ArgumentProcessor::apply_prefix_tags(base, None);
        assert!(result.is_some());

        let tags = result.unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("base1").unwrap()));
    }

    #[test]
    fn given_only_prefix_tags_when_apply_prefix_tags_then_returns_prefix_set() {
        let prefix = {
            let mut set = HashSet::new();
            set.insert(Tag::new("prefix1").unwrap());
            Some(set)
        };

        let result = ArgumentProcessor::apply_prefix_tags(None, prefix);
        assert!(result.is_some());

        let tags = result.unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("prefix1").unwrap()));
    }

    #[test]
    fn given_no_tags_when_apply_prefix_tags_then_returns_none() {
        let result = ArgumentProcessor::apply_prefix_tags(None, None);
        assert!(result.is_none());
    }

    #[test]
    fn given_valid_tag_string_when_parse_tags_with_error_handling_then_returns_ok_result() {
        let tag_str = Some("tag1,tag2".to_string());
        let result = ArgumentProcessor::parse_tags_with_error_handling(&tag_str);

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn given_none_tag_string_when_parse_tags_with_error_handling_then_returns_ok_none() {
        let result = ArgumentProcessor::parse_tags_with_error_handling(&None);

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert!(tags.is_empty());
    }
}
