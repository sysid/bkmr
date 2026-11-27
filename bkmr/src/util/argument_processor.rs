use crate::cli::args::TagFilterArgs;
use crate::cli::error::{CliError, CliResult};
use crate::domain::tag::Tag;
use std::collections::HashSet;
use tracing::instrument;

/// Utility for processing and parsing common CLI argument patterns
pub struct ArgumentProcessor;

impl ArgumentProcessor {
    /// Parse a tag string into a HashSet of Tag objects
    /// This centralizes the tag parsing logic used across multiple commands.
    /// An invalid tag is an error: dropping it, or the whole filter, would silently widen
    /// (exclusions) or narrow (inclusions) the results.
    /// Input without any tag (`","`, `" "`) is no filter: an empty set would still count as one.
    #[instrument(level = "trace")]
    pub fn parse_tag_string(tag_str: &Option<String>) -> CliResult<Option<HashSet<Tag>>> {
        let tags = Tag::parse_tag_option(tag_str.as_deref()).map_err(|e| {
            CliError::InvalidInput(format!(
                "Invalid tags '{}': {}",
                tag_str.as_deref().unwrap_or_default(),
                e
            ))
        })?;
        Ok(tags.filter(|t| !t.is_empty()))
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
        Ok(Self::parse_tag_string(tag_str)?.unwrap_or_default())
    }

    /// Process all search-related tag parameters with prefixes
    /// This centralizes the complex tag processing logic from the search command
    #[instrument(level = "trace")]
    pub fn process_search_tag_parameters(tags: &TagFilterArgs) -> CliResult<SearchTagParams> {
        let merged = |base: &Option<String>, prefix: &Option<String>| -> CliResult<_> {
            Ok(Self::apply_prefix_tags(
                Self::parse_tag_string(base)?,
                Self::parse_tag_string(prefix)?,
            ))
        };
        Ok(SearchTagParams {
            exact_tags: merged(&tags.tags_exact, &tags.tags_exact_prefix)?,
            all_tags: merged(&tags.tags_all, &tags.tags_all_prefix)?,
            all_not_tags: merged(&tags.tags_all_not, &tags.tags_all_not_prefix)?,
            any_tags: merged(&tags.tags_any, &tags.tags_any_prefix)?,
            any_not_tags: merged(&tags.tags_any_not, &tags.tags_any_not_prefix)?,
        })
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
        let result = ArgumentProcessor::parse_tag_string(&tag_str).unwrap();

        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn given_none_tag_string_when_parse_tag_string_then_returns_none() {
        let result = ArgumentProcessor::parse_tag_string(&None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_invalid_tag_when_parse_tag_string_then_error_names_the_input() {
        let tag_str = Some("good,bad tag".to_string());

        let error = ArgumentProcessor::parse_tag_string(&tag_str).unwrap_err();

        assert!(matches!(error, CliError::InvalidInput(_)), "got: {error:?}");
        assert!(error.to_string().contains("bad tag"), "{error}");
    }

    #[test]
    fn given_only_separators_when_parse_tag_string_then_returns_no_filter() {
        // An empty set would count as a tag filter and switch hsearch to its filtered paths
        for input in [",", " , ", " "] {
            let result = ArgumentProcessor::parse_tag_string(&Some(input.to_string())).unwrap();

            assert!(result.is_none(), "{input:?} gave {result:?}");
        }
    }

    #[test]
    fn given_invalid_tag_when_parsing_for_add_or_search_then_both_report_the_same_error() {
        let tag_str = Some("good,bad tag".to_string());

        let for_add = ArgumentProcessor::parse_tags_with_error_handling(&tag_str).unwrap_err();
        let for_search = ArgumentProcessor::parse_tag_string(&tag_str).unwrap_err();

        assert_eq!(for_add.to_string(), for_search.to_string());
    }

    #[test]
    fn given_invalid_exclude_tag_when_processing_search_tags_then_error_instead_of_dropping_filter()
    {
        // Dropping an exclusion filter silently would widen the results
        let result = ArgumentProcessor::process_search_tag_parameters(&TagFilterArgs {
            tags_any_not: Some("_imported_,bad tag".to_string()),
            ..Default::default()
        });

        assert!(result.is_err(), "got: {result:?}");
    }

    #[test]
    fn given_invalid_prefix_tag_when_processing_search_tags_then_error() {
        let result = ArgumentProcessor::process_search_tag_parameters(&TagFilterArgs {
            tags_all: Some("rust".to_string()),
            tags_all_prefix: Some("bad tag".to_string()),
            ..Default::default()
        });

        assert!(result.is_err(), "got: {result:?}");
    }

    #[test]
    fn given_base_and_prefix_when_processing_search_tags_then_each_filter_merges_both() {
        let params = ArgumentProcessor::process_search_tag_parameters(&TagFilterArgs {
            tags_any_not: Some("draft".to_string()),
            tags_any_not_prefix: Some("_imported_".to_string()),
            ..Default::default()
        })
        .unwrap();

        let excluded = params.any_not_tags.unwrap();
        assert_eq!(excluded.len(), 2);
        assert!(excluded.contains(&Tag::new("draft").unwrap()));
        assert!(excluded.contains(&Tag::new("_imported_").unwrap()));
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
