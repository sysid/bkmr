// src/application/actions/shell_action.rs
use crate::application::services::interpolation::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use rustyline::{config::Configurer, error::ReadlineError, history::FileHistory, EditMode, Editor};
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct ShellAction {
    interpolation_service: Arc<dyn InterpolationService>,
    interactive: bool,
}

impl ShellAction {
    pub fn new(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
            // todo: make this configurable via CLI or environment variable
            interactive: true, // Default to interactive mode
        }
    }
    #[allow(dead_code)]
    pub fn new_direct(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
            interactive: false, // Default to non-interactive/legacy mode
        }
    }

    fn interactive_execute(&self, script: &str) -> DomainResult<()> {
        // Configure rustyline to mimic user's shell environment
        let mut rl = self.create_configured_editor()?;
        
        // Present the script for interactive editing with pre-filled content
        match rl.readline_with_initial("Execute: ", (script, "")) {
            Ok(final_command) => {
                if final_command.trim().is_empty() {
                    debug!("User provided empty command, skipping execution");
                    return Ok(());
                }
                
                // Add the command to rustyline's history
                let _ = rl.add_history_entry(&final_command);
                
                // Save history to file for persistence across sessions
                if let Err(e) = rl.save_history(&self.get_history_file_path()) {
                    debug!("Failed to save history: {}", e);
                }
                
                debug!("Executing interactive command: {}", final_command);
                self.execute_script(&final_command)
            }
            Err(ReadlineError::Interrupted) => {
                debug!("User cancelled shell execution with Ctrl-C");
                Ok(())
            }
            Err(e) => Err(DomainError::Other(format!("Readline error: {}", e))),
        }
    }

    fn create_configured_editor(&self) -> DomainResult<Editor<(), FileHistory>> {
        // Create editor with default config first
        let mut rl = Editor::new()
            .map_err(|e| DomainError::Other(format!("Failed to create readline editor: {}", e)))?;
        
        // Configure the editor to mimic shell behavior
        rl.set_auto_add_history(true);
        rl.set_history_ignore_space(true);
        let _ = rl.set_history_ignore_dups(true);
        rl.set_edit_mode(self.detect_edit_mode());
        
        // Load existing history if available
        let history_file = self.get_history_file_path();
        if let Err(e) = rl.load_history(&history_file) {
            debug!("No existing history file or failed to load: {}", e);
        }
        
        Ok(rl)
    }

    fn detect_edit_mode(&self) -> EditMode {
        // Check for vim mode indicators in common shell configurations
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                // Check for zsh vi mode
                if std::env::var("ZSH_VI_MODE").is_ok() {
                    return EditMode::Vi;
                }
            }
        }
        
        // Check for readline configuration
        if let Ok(inputrc) = std::env::var("INPUTRC") {
            if let Ok(content) = std::fs::read_to_string(&inputrc) {
                if content.contains("set editing-mode vi") {
                    return EditMode::Vi;
                }
            }
        }
        
        // Check default inputrc locations
        if let Some(home_dir) = dirs::home_dir() {
            let inputrc_path = home_dir.join(".inputrc");
            if let Ok(content) = std::fs::read_to_string(&inputrc_path) {
                if content.contains("set editing-mode vi") {
                    return EditMode::Vi;
                }
            }
        }
        
        // Check bash vi mode environment variable
        if std::env::var("BASH_VI_MODE").is_ok() {
            return EditMode::Vi;
        }
        
        // Default to emacs mode (readline default)
        EditMode::Emacs
    }
    
    fn get_history_file_path(&self) -> std::path::PathBuf {
        // Use a bkmr-specific history file in the user's config directory
        if let Some(config_dir) = dirs::config_dir() {
            let bkmr_dir = config_dir.join("bkmr");
            std::fs::create_dir_all(&bkmr_dir).ok(); // Create directory if it doesn't exist
            bkmr_dir.join("shell_history.txt")
        } else {
            // Fallback to temp directory
            std::env::temp_dir().join("bkmr_shell_history.txt")
        }
    }

    fn execute_script(&self, script: &str) -> DomainResult<()> {
        // Create a temporary file to store the script
        let mut temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| DomainError::Other(format!("Failed to create temporary file: {}", e)))?;

        // Write the script to the temporary file
        temp_file
            .write_all(script.as_bytes())
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

        // Execute the script
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        let status = Command::new(&shell)
            .arg(temp_file.path())
            .status()
            .map_err(|e| DomainError::Other(format!("Failed to execute shell script: {}", e)))?;

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
}

impl BookmarkAction for ShellAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the shell script content (stored in URL field)
        let script = &bookmark.url;

        // Apply any interpolation if the script contains template variables
        let rendered_script = if script.contains("{{") || script.contains("{%") {
            self.interpolation_service
                .render_bookmark_url(bookmark)
                .map_err(|e| DomainError::Other(format!("Failed to render shell script: {}", e)))?
        } else {
            script.to_string()
        };

        debug!("Shell script (interactive={}): {}", self.interactive, rendered_script);

        // Print a header to indicate what's being executed
        // eprintln!("Executing: {}", bookmark.title);
        eprintln!("---");

        let result = if self.interactive {
            self.interactive_execute(&rendered_script)
        } else {
            self.execute_script(&rendered_script)
        };

        // Print a footer after execution
        eprintln!("---");

        result
    }

    fn description(&self) -> &'static str {
        "Execute as shell script"
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
    fn test_shell_action_executes_script_direct() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(interpolation_service);

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
            created_at: Some(chrono::Utc::now()),
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
    fn test_shell_action_with_interpolation_direct() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor.clone()));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(interpolation_service);

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
            created_at: Some(chrono::Utc::now()),
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
    fn test_shell_action_with_failing_script_direct() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(interpolation_service);

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
            created_at: Some(chrono::Utc::now()),
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

    #[test]
    fn test_shell_action_interactive_mode_defaults() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        
        // Act
        let interactive_action = ShellAction::new(interpolation_service.clone());
        let direct_action = ShellAction::new_direct(interpolation_service);
        
        // Assert
        assert!(interactive_action.interactive, "new() should default to interactive mode");
        assert!(!direct_action.interactive, "new_direct() should set non-interactive mode");
    }

    #[test]
    fn test_execute_script_method() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(interpolation_service);
        
        // Act
        let result = action.execute_script("echo 'test execute_script method'");
        
        // Assert
        assert!(result.is_ok(), "execute_script should work directly");
    }

    #[test]
    fn test_execute_script_with_parameters() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(interpolation_service);
        
        // Act - This simulates what would happen after interactive editing
        let script_with_params = "echo 'Hello' && echo 'World' && echo 'Parameters work!'";
        let result = action.execute_script(script_with_params);
        
        // Assert
        assert!(result.is_ok(), "Shell script with parameters should execute successfully");
    }

    #[test]
    fn test_detect_edit_mode_default() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(interpolation_service);
        
        // Act
        let edit_mode = action.detect_edit_mode();
        
        // Assert - Should be either Emacs or Vi mode (both are valid)
        assert!(
            matches!(edit_mode, EditMode::Emacs) || matches!(edit_mode, EditMode::Vi),
            "Should detect either Emacs or Vi mode, got: {:?}", 
            edit_mode
        );
    }

    #[test]
    fn test_get_history_file_path() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(interpolation_service);
        
        // Act
        let history_path = action.get_history_file_path();
        
        // Assert
        assert!(history_path.to_string_lossy().contains("shell_history.txt"), "Should create a history file path");
    }

    #[test]
    fn test_create_configured_editor() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(interpolation_service);
        
        // Act
        let result = action.create_configured_editor();
        
        // Assert
        assert!(result.is_ok(), "Should successfully create configured editor");
    }
}
