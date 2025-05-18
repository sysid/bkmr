// src/application/services/impl/tag_service_impl.rs
use std::sync::Arc;

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::services::tag_service::TagService;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::tag::Tag;
use tracing::{debug, instrument};

pub struct TagServiceImpl<R: BookmarkRepository> {
    repository: Arc<R>,
}

impl<R: BookmarkRepository> TagServiceImpl<R> {
    pub fn new(repository: Arc<R>) -> Self {
        debug!("Creating new TagServiceImpl");
        Self { repository }
    }
}

impl<R: BookmarkRepository> TagService for TagServiceImpl<R> {
    #[instrument(skip(self), level = "debug", fields(repo_type = std::any::type_name::<R>()))]
    fn get_all_tags(&self) -> ApplicationResult<Vec<(Tag, usize)>> {
        let tags = self.repository.get_all_tags()?;
        Ok(tags)
    }

    #[instrument(skip(self), level = "debug", fields(tag = %tag.value()))]
    fn get_related_tags(&self, tag: &Tag) -> ApplicationResult<Vec<(Tag, usize)>> {
        let related_tags = self.repository.get_related_tags(tag)?;
        Ok(related_tags)
    }

    #[instrument(skip(self), level = "debug", fields(tag_str = %tag_str))]
    fn parse_tag_string(&self, tag_str: &str) -> ApplicationResult<Vec<Tag>> {
        match Tag::parse_tag_str(tag_str) {
            Ok(Some(tag_set)) => Ok(tag_set.into_iter().collect()),
            Ok(None) => Ok(Vec::new()),
            Err(e) => Err(ApplicationError::Validation(format!(
                "Invalid tag string: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};
    use serial_test::serial;
    use std::collections::HashSet;

    // Helper function to create a TagServiceImpl with a test repository
    fn create_test_service() -> impl TagService {
        let repository = setup_test_db();
        let arc_repository = Arc::new(repository);
        TagServiceImpl::new(arc_repository)
    }

    #[test]
    #[serial]
    fn given_test_database_when_get_all_tags_then_returns_all_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let tag_counts = service.get_all_tags().unwrap();

        // Assert
        // Based on up.sql data, we expect these tags: aaa, bbb, ccc, xxx, yyy
        assert!(!tag_counts.is_empty(), "Tag list should not be empty");

        // Check that the expected tags are present
        let tags: HashSet<String> = tag_counts
            .iter()
            .map(|(tag, _)| tag.value().to_string())
            .collect();

        assert!(tags.contains("aaa"), "Tag 'aaa' should be present");
        assert!(tags.contains("bbb"), "Tag 'bbb' should be present");
        assert!(tags.contains("ccc"), "Tag 'ccc' should be present");
        assert!(tags.contains("xxx"), "Tag 'xxx' should be present");
        assert!(tags.contains("yyy"), "Tag 'yyy' should be present");

        // Verify that tag counts are reasonable
        let aaa_count = tag_counts
            .iter()
            .find(|(tag, _)| tag.value() == "aaa")
            .map(|(_, count)| *count)
            .unwrap_or(0);

        // From up.sql, 'aaa' appears in 4 records
        assert_eq!(aaa_count, 4, "Tag 'aaa' should appear in 4 bookmarks");
    }

    #[test]
    #[serial]
    fn given_test_database_when_get_related_tags_for_ccc_then_returns_correct_related_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let ccc_tag = Tag::new("ccc").unwrap();

        // Act
        let related_tags = service.get_related_tags(&ccc_tag).unwrap();

        // Assert
        let related_tag_names: HashSet<String> = related_tags
            .iter()
            .map(|(tag, _)| tag.value().to_string())
            .collect();

        // Based on up.sql, 'ccc' co-occurs with 'aaa', 'bbb', 'xxx', 'yyy'
        assert!(
            related_tag_names.contains("aaa"),
            "Tag 'aaa' should be related to 'ccc'"
        );
        assert!(
            related_tag_names.contains("bbb"),
            "Tag 'bbb' should be related to 'ccc'"
        );
        assert!(
            related_tag_names.contains("yyy"),
            "Tag 'yyy' should be related to 'ccc'"
        );

        // Check the most frequent co-occurrence
        let aaa_count = related_tags
            .iter()
            .find(|(tag, _)| tag.value() == "aaa")
            .map(|(_, count)| *count)
            .unwrap_or(0);

        // From up.sql, 'aaa' co-occurs with 'ccc' in 2 records
        assert_eq!(
            aaa_count, 2,
            "Tag 'aaa' should co-occur with 'ccc' in 2 bookmarks"
        );
    }

    #[test]
    #[serial]
    fn given_empty_tag_when_get_related_tags_then_returns_empty_list() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let non_existent_tag = Tag::new("nonexistent").unwrap();

        // Act
        let related_tags = service.get_related_tags(&non_existent_tag).unwrap();

        // Assert
        assert!(
            related_tags.is_empty(),
            "No tags should be related to a non-existent tag"
        );
    }

    #[test]
    #[serial]
    fn given_valid_tag_string_when_parse_tag_string_then_returns_correct_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let tag_string = "tag1,tag2,tag3";

        // Act
        let tags = service.parse_tag_string(tag_string).unwrap();

        // Assert
        assert_eq!(tags.len(), 3, "Should parse 3 tags");

        let tag_values: Vec<String> = tags.iter().map(|t| t.value().to_string()).collect();
        assert!(tag_values.contains(&"tag1".to_string()));
        assert!(tag_values.contains(&"tag2".to_string()));
        assert!(tag_values.contains(&"tag3".to_string()));
    }

    #[test]
    #[serial]
    fn given_empty_tag_string_when_parse_tag_string_then_returns_empty_vec() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();

        // Act
        let tags = service.parse_tag_string("").unwrap();

        // Assert
        assert!(
            tags.is_empty(),
            "Empty tag string should return empty vector"
        );
    }

    #[test]
    #[serial]
    fn given_invalid_tag_string_when_parse_tag_string_then_returns_error() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let invalid_tag_string = "valid,not valid with space,another";

        // Act
        let result = service.parse_tag_string(invalid_tag_string);

        // Assert
        assert!(result.is_err(), "Invalid tag string should return error");
        match result {
            Err(ApplicationError::Validation(msg)) => {
                assert!(
                    msg.contains("Invalid tag string"),
                    "Error should mention invalid tag string"
                );
            }
            _ => panic!("Expected a Validation error"),
        }
    }

    #[test]
    #[serial]
    fn given_tag_string_with_duplicates_when_parse_tag_string_then_returns_unique_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let service = create_test_service();
        let tag_string_with_duplicates = "dup,dup,unique";

        // Act
        let tags = service
            .parse_tag_string(tag_string_with_duplicates)
            .unwrap();

        // Assert
        assert_eq!(tags.len(), 2, "Duplicate tags should be eliminated");

        let tag_values: Vec<String> = tags.iter().map(|t| t.value().to_string()).collect();
        assert!(tag_values.contains(&"dup".to_string()));
        assert!(tag_values.contains(&"unique".to_string()));
    }
}
