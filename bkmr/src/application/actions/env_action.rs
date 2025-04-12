// src/application/actions/env_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct EnvAction {
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl EnvAction {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self { interpolation_engine }
    }
}

impl BookmarkAction for EnvAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the environment variables content (stored in URL field)
        let env_content = &bookmark.url;

        // Apply any interpolation if the content contains template variables
        let rendered_content = if env_content.contains("{{") || env_content.contains("{%") {
            self.interpolation_engine.render_bookmark_url(bookmark)?
        } else {
            env_content.to_string()
        };

        debug!("Printing environment variables to stdout for sourcing");

        // Simply print the content to stdout for sourcing
        // This allows usage like: eval "$(bkmr open 123)"
        // or source <(bkmr open 123)
        println!("{}", rendered_content);

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Print environment variables for sourcing"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
    use std::collections::HashSet;

    #[test]
    fn test_env_action_prints_content() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = EnvAction::new(interpolation_engine);

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
            created_at: chrono::Utc::now(),
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
        let action = EnvAction::new(interpolation_engine);

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
            created_at: chrono::Utc::now(),
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