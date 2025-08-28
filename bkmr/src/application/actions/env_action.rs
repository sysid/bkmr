// src/application/actions/env_action.rs
use crate::application::services::TemplateService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use crate::util::interpolation::InterpolationHelper;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct EnvAction {
    template_service: Arc<dyn TemplateService>,
}

impl EnvAction {
    pub fn new(template_service: Arc<dyn TemplateService>) -> Self {
        Self { template_service }
    }
}

impl BookmarkAction for EnvAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the environment variables content (stored in URL field)
        let env_content = &bookmark.url;

        // Apply any interpolation if the content contains template variables
        let rendered_content = InterpolationHelper::render_if_needed(
            env_content,
            bookmark,
            &self.template_service,
            "environment variables",
        )?;

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
    use crate::application::services::TemplateServiceImpl;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SafeShellExecutor,
    };
    use std::collections::HashSet;

    #[test]
    fn given_env_bookmark_when_execute_then_prints_content() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = EnvAction::new(template_service);

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
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Env action execution should succeed");
        // Note: In a real test, we'd capture stdout to verify output
    }

    #[test]
    fn given_env_with_template_when_execute_then_interpolates_and_prints() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = EnvAction::new(template_service);

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
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Env action execution should succeed");
    }
}
