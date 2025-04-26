// src/application/actions/env_action.rs
use crate::application::services::interpolation::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct EnvAction {
    interpolation_service: Arc<dyn InterpolationService>,
}

impl EnvAction {
    pub fn new(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
        }
    }
}

impl BookmarkAction for EnvAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the environment variables content (stored in URL field)
        let env_content = &bookmark.url;

        // Apply any interpolation if the content contains template variables
        let rendered_content = if env_content.contains("{{") || env_content.contains("{%") {
            self.interpolation_service
                .render_bookmark_url(bookmark)
                .map_err(|e| {
                    DomainError::Other(format!("Failed to render environment variables: {}", e))
                })?
        } else {
            env_content.to_string()
        };

        debug!("Printing environment variables to stdout for sourcing");

        // Add a header to indicate what's being printed
        println!("# Environment variables from: {}", bookmark.title);
        println!(
            "# Usage: eval \"$(bkmr open {})\" or source <(bkmr open {})",
            bookmark.id.unwrap_or(0),
            bookmark.id.unwrap_or(0)
        );
        println!("# ----- BEGIN ENVIRONMENT VARIABLES -----");

        // Print the content with clean formatting
        println!("{}", rendered_content);

        // Add a footer
        println!("# ----- END ENVIRONMENT VARIABLES -----");

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Print environment variables for sourcing"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::services::interpolation::InterpolationServiceImpl;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SafeShellExecutor,
    };
    use std::collections::HashSet;

    #[test]
    fn test_env_action_prints_content() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = EnvAction::new(interpolation_service);

        // Create a simple environment variables content
        let env_content = "export FOO=bar\nexport BAZ=qux";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_env_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: env_content.to_string(),
            title: "Test Environment Variables".to_string(),
            description: "A test set of environment variables".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Env action execution should succeed");
        // Note: In a real test, we'd capture stdout to verify output
    }

    #[test]
    fn test_env_action_with_interpolation() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = EnvAction::new(interpolation_service);

        // Create env content with interpolation
        let env_content = "export DATE={{ current_date | strftime(\"%Y-%m-%d\") }}\nexport USER={{ \"whoami\" | shell }}";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_env_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: env_content.to_string(),
            title: "Test Env with Interpolation".to_string(),
            description: "A test set of environment variables with interpolation".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Env action execution should succeed");
    }
}
