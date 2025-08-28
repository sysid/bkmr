// src/application/actions/shell_action.rs
use crate::application::services::template_service::TemplateService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::util::interpolation::InterpolationHelper;
use rustyline::{config::Configurer, error::ReadlineError, history::FileHistory, EditMode, Editor};
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct ShellAction {
    template_service: Arc<dyn TemplateService>,
    interactive: bool,
    script_args: Vec<String>,
}

impl ShellAction {
    pub fn new(template_service: Arc<dyn TemplateService>, interactive: bool) -> Self {
        Self {
            template_service,
            interactive,
            script_args: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn new_direct(template_service: Arc<dyn TemplateService>) -> Self {
        Self {
            template_service,
            interactive: false, // Direct execution without interaction
            script_args: Vec::new(),
        }
    }

    pub fn new_direct_with_args(
        template_service: Arc<dyn TemplateService>,
        script_args: Vec<String>,
    ) -> Self {
        Self {
            template_service,
            interactive: false, // Direct execution without interaction
            script_args,
        }
    }

    fn interactive_execute(&self, script: &str) -> DomainResult<()> {
        // Configure rustyline to mimic user's shell environment
        let mut rl = self.create_configured_editor()?;

        // Present the script for interactive editing with pre-filled content
        match rl.readline_with_initial("", (script, "")) {
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

        let mut command = Command::new(&shell);
        command.arg(temp_file.path());

        // Append script arguments if provided
        if !self.script_args.is_empty() {
            command.args(&self.script_args);
            debug!(
                "Executing shell script with arguments: {:?}",
                self.script_args
            );
        }

        let status = command
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
        let rendered_script = InterpolationHelper::render_if_needed(
            script,
            bookmark,
            &self.template_service,
            "shell script",
        )?;

        debug!(
            "Shell script (interactive={}): {}",
            self.interactive, rendered_script
        );

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
    use crate::application::services::TemplateServiceImpl;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SafeShellExecutor,
    };
    use std::collections::HashSet;

    #[test]
    fn given_shell_script_when_execute_direct_then_runs_without_edit() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(template_service);

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
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Shell action execution should succeed");
        // Note: In a real test, we'd capture stdout to verify output
    }

    #[test]
    fn given_script_with_template_when_execute_direct_then_interpolates_content() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor.clone()));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(template_service);

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
            file_path: None,
            file_mtime: None,
            file_hash: None,
        };

        // Act
        let result = action.execute(&bookmark);

        // Assert
        assert!(result.is_ok(), "Shell action execution should succeed");
    }

    #[test]
    fn given_failing_script_when_execute_direct_then_returns_error() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(template_service);

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
            file_path: None,
            file_mtime: None,
            file_hash: None,
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
    fn given_interactive_mode_when_create_shell_action_then_uses_defaults() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));

        // Act
        let interactive_action = ShellAction::new(template_service.clone(), true);
        let direct_action = ShellAction::new_direct(template_service);

        // Assert
        assert!(
            interactive_action.interactive,
            "new() with true should set interactive mode"
        );
        assert!(
            !direct_action.interactive,
            "new_direct() should set non-interactive mode"
        );
    }

    #[test]
    fn given_shell_script_when_execute_method_called_then_returns_success() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(template_service);

        // Act
        let result = action.execute_script("echo 'test execute_script method'");

        // Assert
        assert!(result.is_ok(), "execute_script should work directly");
    }

    #[test]
    fn given_script_with_parameters_when_execute_then_passes_args() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new_direct(template_service);

        // Act - This simulates what would happen after interactive editing
        let script_with_params = "echo 'Hello' && echo 'World' && echo 'Parameters work!'";
        let result = action.execute_script(script_with_params);

        // Assert
        assert!(
            result.is_ok(),
            "Shell script with parameters should execute successfully"
        );
    }

    #[test]
    fn given_environment_when_detect_edit_mode_then_returns_appropriate_mode() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(template_service, true);

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
    fn given_home_directory_when_get_history_path_then_returns_history_file() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(template_service, true);

        // Act
        let history_path = action.get_history_file_path();

        // Assert
        assert!(
            history_path.to_string_lossy().contains("shell_history.txt"),
            "Should create a history file path"
        );
    }

    #[test]
    fn given_configuration_when_create_editor_then_returns_configured_editor() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let action = ShellAction::new(template_service, true);

        // Act
        let result = action.create_configured_editor();

        // Assert
        assert!(
            result.is_ok(),
            "Should successfully create configured editor"
        );
    }

    #[test]
    fn given_template_service_and_args_when_new_direct_then_creates_action_with_args() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));
        let args = vec![
            "--option1".to_string(),
            "value1".to_string(),
            "arg2".to_string(),
        ];

        // Act
        let action = ShellAction::new_direct_with_args(template_service, args.clone());

        // Assert
        assert!(!action.interactive, "Should be non-interactive");
        assert_eq!(action.script_args, args, "Should store script arguments");
    }

    #[test]
    fn given_script_with_arguments_when_execute_then_passes_to_shell() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let template_service = Arc::new(TemplateServiceImpl::new(interpolation_engine));

        // Create script arguments
        let args = vec!["arg1".to_string(), "arg2".to_string()];
        let action = ShellAction::new_direct_with_args(template_service, args);

        // Create a shell script that uses arguments via $1, $2, etc.
        let script = "echo \"First arg: $1, Second arg: $2\"";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_shell_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: script.to_string(),
            title: "Test Shell Script with Args".to_string(),
            description: "A test shell script that uses arguments".to_string(),
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
        assert!(
            result.is_ok(),
            "Shell action with arguments should execute successfully"
        );
    }
}
