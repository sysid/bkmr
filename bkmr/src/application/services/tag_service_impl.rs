// src/application/services/impl/tag_service_impl.rs
use std::sync::Arc;

use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::services::tag_service::TagService;
use crate::domain::error_context::ApplicationErrorContext;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::domain::tag::Tag;
use tracing::instrument;

pub struct TagServiceImpl<R: BookmarkRepository> {
    repository: Arc<R>,
}

impl<R: BookmarkRepository> TagServiceImpl<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }
}

impl<R: BookmarkRepository> TagService for TagServiceImpl<R> {
    #[instrument(skip(self), level = "debug", fields(repo_type = std::any::type_name::<R>()))]
    fn get_all_tags(&self) -> ApplicationResult<Vec<(Tag, usize)>> {
        let tags = self
            .repository
            .get_all_tags()
            .app_context("retrieving all tags")?;
        Ok(tags)
    }

    #[instrument(skip(self), level = "debug", fields(tag = %tag.value()))]
    fn get_related_tags(&self, tag: &Tag) -> ApplicationResult<Vec<(Tag, usize)>> {
        let related_tags = self
            .repository
            .get_related_tags(tag)
            .app_context("retrieving related tags")?;
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
    use crate::domain::bookmark::Bookmark;
    use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};
    use std::collections::HashSet;

    // Helper function to create a TagServiceImpl with a test repository
    fn create_test_service() -> impl TagService {
        let repository = setup_test_db();
        let arc_repository = Arc::new(repository);
        TagServiceImpl::new(arc_repository)
    }

    #[test]
    fn given_tagged_bookmarks_when_get_all_tags_then_returns_all_tags_with_counts() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = Arc::new(setup_test_db());
        for (url, tags) in [
            ("https://one.example.com", "aaa,ccc"),
            ("https://two.example.com", "aaa,bbb,ccc"),
            ("https://three.example.com", "ccc,xxx"),
            ("https://four.example.com", "yyy"),
        ] {
            let mut bookmark =
                Bookmark::new(url, "title", "", Tag::parse_tags(tags).unwrap()).unwrap();
            repository.add(&mut bookmark).unwrap();
        }
        let service = TagServiceImpl::new(repository);

        // Act
        let tag_counts = service.get_all_tags().unwrap();

        // Assert
        let counts: HashSet<(String, usize)> = tag_counts
            .iter()
            .map(|(tag, count)| (tag.value().to_string(), *count))
            .collect();
        let expected: HashSet<(String, usize)> =
            [("aaa", 2), ("bbb", 1), ("ccc", 3), ("xxx", 1), ("yyy", 1)]
                .iter()
                .map(|(tag, count)| (tag.to_string(), *count))
                .collect();
        assert_eq!(counts, expected);
    }

    #[test]
    fn given_tagged_bookmarks_when_get_related_tags_for_ccc_then_returns_cooccurring_tags() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = Arc::new(setup_test_db());
        for (url, tags) in [
            ("https://one.example.com", "aaa,ccc"),
            ("https://two.example.com", "aaa,bbb,ccc"),
            ("https://three.example.com", "ccc,xxx"),
            ("https://four.example.com", "yyy"),
        ] {
            let mut bookmark =
                Bookmark::new(url, "title", "", Tag::parse_tags(tags).unwrap()).unwrap();
            repository.add(&mut bookmark).unwrap();
        }
        let service = TagServiceImpl::new(repository);
        let ccc_tag = Tag::new("ccc").unwrap();

        // Act
        let related_tags = service.get_related_tags(&ccc_tag).unwrap();

        // Assert: 'yyy' never appears together with 'ccc'
        let related: HashSet<(String, usize)> = related_tags
            .iter()
            .map(|(tag, count)| (tag.value().to_string(), *count))
            .collect();
        let expected: HashSet<(String, usize)> = [("aaa", 2), ("bbb", 1), ("xxx", 1)]
            .iter()
            .map(|(tag, count)| (tag.to_string(), *count))
            .collect();
        assert_eq!(related, expected);
    }

    #[test]
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
