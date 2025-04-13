// src/application/actions/shell_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct ShellAction {
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl ShellAction {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            interpolation_engine,
        }
    }
}

impl BookmarkAction for ShellAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the shell script content (stored in URL field)
        let script = &bookmark.url;

        // Apply any interpolation if the script contains template variables
        let rendered_script = if script.contains("{{") || script.contains("{%") {
            self.interpolation_engine.render_bookmark_url(bookmark)?
        } else {
            script.to_string()
        };

        debug!("Executing shell script: {}", rendered_script);

        // Create a temporary file to store the script
        let mut temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| DomainError::Other(format!("Failed to create temporary file: {}", e)))?;

        // Write the script to the temporary file
        temp_file
            .write_all(rendered_script.as_bytes())
            .map_err(|e| DomainError::Other(format!("Failed to write to temporary file: {}", e)))?;

        // Make the temporary script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = temp_file
                .as_file()
                .metadata()
                .map_err(|e| DomainError::Other(format!("Failed to get file metadata: {}", e)))?;
            let mut perms = metadata.permissions();
            perms.set_mode(0o755); // rwx r-x r-x
            std::fs::set_permissions(temp_file.path(), perms).map_err(|e| {
                DomainError::Other(format!("Failed to set file permissions: {}", e))
            })?;
        }

        // Execute the script and capture the output
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        // Print a header to indicate what's being executed
        eprintln!("Executing: {}", bookmark.title);
        eprintln!("---");

        // Execute the command directly with inherited stdio
        let status = Command::new(&shell)
            .arg(temp_file.path())
            .status()
            .map_err(|e| DomainError::Other(format!("Failed to execute shell script: {}", e)))?;

        // Print a footer after execution
        eprintln!("---");

        // Return result based on exit status
        if status.success() {
            Ok(())
        } else {
            Err(DomainError::Other(format!(
                "Shell script exited with non-zero status: {:?}",
                status.code()
            )))
        }
    }

    fn description(&self) -> &'static str {
        "Execute as shell script"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SafeShellExecutor,
    };
    use std::collections::HashSet;

    #[test]
    fn test_shell_action_executes_script() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = ShellAction::new(interpolation_engine);

        // Create a simple shell script that outputs a message
        let script = "echo 'Hello from shell script'";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_shell_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: script.to_string(),
            title: "Test Shell Script".to_string(),
            description: "A test shell script".to_string(),
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
        assert!(result.is_ok(), "Shell action execution should succeed");
        // Note: In a real test, we'd capture stdout to verify output
    }

    #[test]
    fn test_shell_action_with_interpolation() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor.clone()));
        let action = ShellAction::new(interpolation_engine);

        // Create a shell script with interpolation
        let script = "echo 'Current date: {{ current_date | strftime(\"%Y-%m-%d\") }}'";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_shell_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: script.to_string(),
            title: "Test Shell Script with Interpolation".to_string(),
            description: "A test shell script with interpolation".to_string(),
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
        assert!(result.is_ok(), "Shell action execution should succeed");
    }

    #[test]
    fn test_shell_action_with_failing_script() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = ShellAction::new(interpolation_engine);

        // Create a shell script that will fail
        let script = "exit 1";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_shell_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: script.to_string(),
            title: "Test Failing Shell Script".to_string(),
            description: "A test shell script that fails".to_string(),
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
        assert!(result.is_err(), "Shell action execution should fail");
        if let Err(DomainError::Other(msg)) = result {
            assert!(
                msg.contains("non-zero status"),
                "Error should mention non-zero status"
            );
        } else {
            panic!("Expected DomainError::Other");
        }
    }
}
