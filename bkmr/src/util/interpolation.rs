use crate::application::services::TemplateService;
use crate::domain::{bookmark::Bookmark, error::DomainError, error::DomainResult};
use std::sync::Arc;

/// Utility for handling common interpolation patterns across actions
pub struct InterpolationHelper;

impl InterpolationHelper {
    /// Renders bookmark content if it contains template variables, otherwise returns the content as-is
    ///
    /// # Arguments
    /// * `content` - The content to potentially interpolate
    /// * `bookmark` - The bookmark context for interpolation
    /// * `service` - The template service to use
    /// * `context_name` - Name of the context for error messages (e.g., "shell script", "snippet")
    ///
    /// # Returns
    /// * `Ok(String)` - The rendered content (interpolated or original)
    /// * `Err(DomainError)` - If interpolation fails
    pub fn render_if_needed(
        content: &str,
        bookmark: &Bookmark,
        service: &Arc<dyn TemplateService>,
        context_name: &str,
    ) -> DomainResult<String> {
        if content.contains("{{") || content.contains("{%") {
            service.render_bookmark_url(bookmark).map_err(|e| {
                DomainError::Other(format!("Failed to render {}: {}", context_name, e))
            })
        } else {
            Ok(content.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::error::{ApplicationError, ApplicationResult};
    use crate::application::services::TemplateService;
    use crate::domain::bookmark::Bookmark;
    use std::sync::Arc;

    #[derive(Debug)]
    struct MockInterpolationService {
        should_fail: bool,
    }

    impl TemplateService for MockInterpolationService {
        fn edit_bookmark_with_template(
            &self,
            _bookmark: Option<Bookmark>,
        ) -> ApplicationResult<(Bookmark, bool)> {
            unimplemented!("Not needed for interpolation helper tests")
        }

        fn render_bookmark_url(&self, _bookmark: &Bookmark) -> ApplicationResult<String> {
            if self.should_fail {
                Err(ApplicationError::Other(
                    "Mock interpolation error".to_string(),
                ))
            } else {
                Ok("rendered content".to_string())
            }
        }
    }

    fn create_test_bookmark() -> Bookmark {
        use chrono::Utc;
        Bookmark::from_storage(
            1,
            "test content".to_string(),
            "Test Bookmark".to_string(),
            "Test description".to_string(),
            "".to_string(), // tag string
            0,              // access count
            Some(Utc::now()),
            Utc::now(),
            None,  // embedding
            None,  // content hash
            false, // embeddable
            None,  // file_path
            None,  // file_mtime
            None,  // file_hash
        )
        .unwrap()
    }

    #[test]
    fn given_simple_content_when_render_if_needed_then_returns_unchanged() {
        let bookmark = create_test_bookmark();
        let service: Arc<dyn TemplateService> =
            Arc::new(MockInterpolationService { should_fail: false });
        let content = "simple content without templates";

        let result =
            InterpolationHelper::render_if_needed(content, &bookmark, &service, "test").unwrap();

        assert_eq!(result, "simple content without templates");
    }

    #[test]
    fn given_template_content_when_render_if_needed_then_returns_interpolated() {
        let bookmark = create_test_bookmark();
        let service: Arc<dyn TemplateService> =
            Arc::new(MockInterpolationService { should_fail: false });
        let content = "content with {{ template }}";

        let result =
            InterpolationHelper::render_if_needed(content, &bookmark, &service, "test").unwrap();

        assert_eq!(result, "rendered content");
    }

    #[test]
    fn given_template_content_when_render_fails_then_returns_original() {
        let bookmark = create_test_bookmark();
        let service: Arc<dyn TemplateService> =
            Arc::new(MockInterpolationService { should_fail: true });
        let content = "content with {{ template }}";

        let result =
            InterpolationHelper::render_if_needed(content, &bookmark, &service, "shell script");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to render shell script"));
    }

    #[test]
    fn given_jinja_syntax_when_render_if_needed_then_handles_correctly() {
        let bookmark = create_test_bookmark();
        let service: Arc<dyn TemplateService> =
            Arc::new(MockInterpolationService { should_fail: false });
        let content = "content with {% if condition %}";

        let result =
            InterpolationHelper::render_if_needed(content, &bookmark, &service, "test").unwrap();

        assert_eq!(result, "rendered content");
    }
}
