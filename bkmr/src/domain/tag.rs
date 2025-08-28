// bkmr/src/domain/tag.rs
use std::collections::HashSet;
use std::fmt;

use crate::domain::error::{DomainError, DomainResult};

/// Represents a single tag as a value object
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag {
    value: String,
}

impl Tag {
    /// Creates a new Tag with validation
    pub fn new<S: AsRef<str>>(value: S) -> DomainResult<Self> {
        let value = value.as_ref().trim().to_lowercase();

        if value.is_empty() {
            return Err(DomainError::InvalidTag("Tag cannot be empty".to_string()));
        }

        if value.contains(',') || value.contains(' ') {
            return Err(DomainError::InvalidTag(
                "Tag cannot contain commas or spaces".to_string(),
            ));
        }

        Ok(Self { value })
    }

    /// Get the tag value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Parse a comma-separated tag string into a set of valid Tags
    pub fn parse_tags<S: AsRef<str>>(tag_str: S) -> DomainResult<HashSet<Tag>> {
        let mut result = HashSet::new();

        for tag_value in tag_str
            .as_ref()
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            result.insert(Tag::new(tag_value)?);
        }

        Ok(result)
    }

    /// Parse an optional string into an `Option<HashSet<Tag>>`.
    ///
    /// Returns `None` if the input is `None` or an empty string.
    /// Otherwise, parses the string into a `HashSet<Tag>` and wraps it in `Some`.
    pub fn parse_tag_option(
        tag_str: Option<impl AsRef<str>>,
    ) -> DomainResult<Option<HashSet<Tag>>> {
        match tag_str {
            None => Ok(None),
            Some(s) => {
                let s = s.as_ref();
                if s.is_empty() {
                    Ok(None)
                } else {
                    Tag::parse_tags(s).map(Some)
                }
            }
        }
    }

    /// Parse a string reference into a HashSet of tags.
    /// Returns `None` if the input string is empty.
    pub fn parse_tag_str(tag_str: impl AsRef<str>) -> DomainResult<Option<HashSet<Tag>>> {
        let s = tag_str.as_ref();
        if s.is_empty() {
            Ok(None)
        } else {
            Tag::parse_tags(s).map(Some)
        }
    }

    /// Format a set of tags into a normalized tag string
    pub fn format_tags(tags: &HashSet<Tag>) -> String {
        let mut tag_values: Vec<_> = tags.iter().map(|tag| tag.value.clone()).collect();

        tag_values.sort();

        if tag_values.is_empty() {
            ",,".to_string()
        } else {
            format!(",{},", tag_values.join(","))
        }
    }

    /// Check if a set of tags contains all of another set of tags
    pub fn contains_all(haystack: &HashSet<Tag>, needles: &HashSet<Tag>) -> bool {
        needles.iter().all(|tag| haystack.contains(tag))
    }

    /// Check if a set of tags contains any of another set of tags
    pub fn contains_any(haystack: &HashSet<Tag>, needles: &HashSet<Tag>) -> bool {
        !needles.is_empty() && needles.iter().any(|tag| haystack.contains(tag))
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn given_valid_tag_value_when_create_tag_then_returns_tag() {
        let tag = Tag::new("test").unwrap();
        assert_eq!(tag.value(), "test");

        // Should normalize case
        let tag = Tag::new("TEST").unwrap();
        assert_eq!(tag.value(), "test");

        // Should trim whitespace
        let tag = Tag::new(" test ").unwrap();
        assert_eq!(tag.value(), "test");
    }

    #[test]
    fn given_invalid_tag_value_when_create_tag_then_returns_error() {
        // Empty tag
        assert!(Tag::new("").is_err());

        // Tag with comma
        assert!(Tag::new("test,tag").is_err());

        // Tag with space
        assert!(Tag::new("test tag").is_err());
    }

    #[test]
    fn given_tag_string_when_parse_tags_then_returns_tag_set() {
        let tags = Tag::parse_tags("tag1,tag2,tag3").unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));

        // Should handle extra commas and whitespace
        let tags = Tag::parse_tags(",tag1,,tag2, tag3,").unwrap();
        assert_eq!(tags.len(), 3);
    }

    #[test]
    fn given_tag_set_when_format_then_returns_formatted_string() {
        let mut tags = HashSet::new();
        tags.insert(Tag::new("tag1").unwrap());
        tags.insert(Tag::new("tag2").unwrap());

        let formatted = Tag::format_tags(&tags);
        assert_eq!(formatted, ",tag1,tag2,");

        // Empty set
        let tags = HashSet::new();
        let formatted = Tag::format_tags(&tags);
        assert_eq!(formatted, ",,");
    }

    #[test]
    fn given_tag_sets_when_contains_all_then_validates_subset() {
        let mut haystack = HashSet::new();
        haystack.insert(Tag::new("tag1").unwrap());
        haystack.insert(Tag::new("tag2").unwrap());
        haystack.insert(Tag::new("tag3").unwrap());

        let mut needles = HashSet::new();
        needles.insert(Tag::new("tag1").unwrap());
        needles.insert(Tag::new("tag2").unwrap());

        assert!(Tag::contains_all(&haystack, &needles));

        // Should return false if any needle is missing
        needles.insert(Tag::new("tag4").unwrap());
        assert!(!Tag::contains_all(&haystack, &needles));
    }

    #[test]
    fn given_tag_sets_when_contains_any_then_validates_intersection() {
        let mut haystack = HashSet::new();
        haystack.insert(Tag::new("tag1").unwrap());
        haystack.insert(Tag::new("tag2").unwrap());

        let mut needles = HashSet::new();
        needles.insert(Tag::new("tag2").unwrap());
        needles.insert(Tag::new("tag3").unwrap());

        assert!(Tag::contains_any(&haystack, &needles));

        // Should return false if no overlap
        let mut needles = HashSet::new();
        needles.insert(Tag::new("tag3").unwrap());
        needles.insert(Tag::new("tag4").unwrap());

        assert!(!Tag::contains_any(&haystack, &needles));

        // Should return false for empty needles
        let needles = HashSet::new();
        assert!(!Tag::contains_any(&haystack, &needles));
    }

    #[test]
    fn given_valid_option_string_when_parse_tag_option_then_returns_tag_set() {
        let result = Tag::parse_tag_option(Some("tag1,tag2,tag3")).unwrap();
        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn given_empty_option_string_when_parse_tag_option_then_returns_empty_set() {
        let result = Tag::parse_tag_option(Some("")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_none_option_when_parse_tag_option_then_returns_empty_set() {
        let result = Tag::parse_tag_option(None::<&str>).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_invalid_option_string_when_parse_tag_option_then_returns_empty_set() {
        let result = Tag::parse_tag_option(Some("invalid tag with space"));
        assert!(result.is_err());
    }

    #[test]
    fn given_valid_tag_string_when_parse_tag_str_then_returns_tag_set() {
        let result = Tag::parse_tag_str("tag1,tag2,tag3").unwrap();
        assert!(result.is_some());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("tag1").unwrap()));
        assert!(tags.contains(&Tag::new("tag2").unwrap()));
        assert!(tags.contains(&Tag::new("tag3").unwrap()));
    }

    #[test]
    fn given_empty_tag_string_when_parse_tag_str_then_returns_empty_set() {
        let result = Tag::parse_tag_str("").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_invalid_tag_string_when_parse_tag_str_then_returns_empty_set() {
        let result = Tag::parse_tag_str("invalid tag with space");
        assert!(result.is_err());
    }
}
