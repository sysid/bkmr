// src/application/services/template_service.rs
use crate::application::error::{ApplicationError, ApplicationResult};
use crate::application::templates::bookmark_template::BookmarkTemplate;
use crate::domain::bookmark::Bookmark;
use crate::domain::error_context::ApplicationErrorContext;
use crate::domain::interpolation::interface::InterpolationEngine;
use crate::domain::system_tag::SystemTag;
use std::fmt::Debug;
use std::fs::{self};
use std::io::Write;
use std::process::Command;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tracing::{debug, instrument};

pub trait TemplateService: Send + Sync + Debug {
    fn edit_bookmark_with_template(
        &self,
        bookmark: Option<Bookmark>,
    ) -> ApplicationResult<(Bookmark, bool)>;

    /// Render an interpolated URL within the context of a bookmark
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String>;
}

#[derive(Debug)]
pub struct TemplateServiceImpl {
    editor: String,
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl Default for TemplateServiceImpl {
    fn default() -> Self {
        // This is used only in tests, create a dummy engine
        use crate::infrastructure::interpolation::minijinja_engine::{
            MiniJinjaEngine, SafeShellExecutor,
        };
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        Self::new(engine)
    }
}

impl TemplateServiceImpl {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        Self {
            editor,
            interpolation_engine,
        }
    }

    pub fn with_editor(editor: String, interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            editor,
            interpolation_engine,
        }
    }
}

impl TemplateService for TemplateServiceImpl {
    /// Opens an editor with a template for creating or updating a bookmark.
    ///
    /// When provided with an existing bookmark (`Some(Bookmark)`), preserves its ID,
    /// timestamps, and embeddings while allowing modification of content.
    /// When provided with `None`, creates a template for a new bookmark.
    ///
    /// Returns the edited/created bookmark and a boolean indicating if the file was modified.
    #[instrument(skip(self, bookmark), level = "debug")]
    fn edit_bookmark_with_template(
        &self,
        bookmark: Option<Bookmark>,
    ) -> ApplicationResult<(Bookmark, bool)> {
        // Create a interpolation from the bookmark or a new empty interpolation
        let template = if let Some(ref bm) = bookmark {
            BookmarkTemplate::from_bookmark(bm)
        } else {
            BookmarkTemplate::for_type(SystemTag::Uri)
        };

        let mut temp_file = NamedTempFile::new().app_context("Failed to create temporary file")?;

        debug!("Temporary file for editing: {:?}", temp_file.path());

        temp_file
            .write_all(template.to_string().as_bytes())
            .app_context("Failed to write to temporary file")?;

        temp_file
            .flush()
            .app_context("Failed to flush temporary file")?;
        let path = temp_file.path().to_path_buf();
        let modified_before = fs::metadata(&path)?.modified()?;

        // Open the editor
        let status = Command::new(&self.editor)
            .arg(temp_file.path())
            .status()
            .app_context("Failed to open editor")?;

        if !status.success() {
            return Err(ApplicationError::Other(
                "Editor exited with error".to_string(),
            ));
        }

        // Get modification time after editing
        let modified_after = fs::metadata(&path)?.modified()?;

        // Check if the file was modified
        let was_modified = modified_after > modified_before;

        // Read the edited file
        let edited_content =
            fs::read_to_string(temp_file.path()).app_context("Failed to read temporary file")?;

        // Parse the interpolation back into a bookmark
        let edited_template = BookmarkTemplate::from_string(&edited_content)?;

        // Convert the interpolation to a bookmark
        let bookmark = edited_template.to_bookmark(bookmark.as_ref())?;

        Ok((bookmark, was_modified))
    }

    #[instrument(level = "debug", skip(self, bookmark))]
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> ApplicationResult<String> {
        self.interpolation_engine
            .render_bookmark_url(bookmark)
            .map_err(|e| ApplicationError::Other(format!("Template rendering error: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::util::testing::init_test_env;
    use std::collections::HashSet;

    // This is more of an integration test and requires manual editing
    // To test locally, enable this test
    // A mock editor function that writes predetermined content to a file
    #[test]
    #[ignore = "Manual test"]
    fn given_bookmark_when_edit_with_template_then_returns_modified_bookmark() {
        let _ = init_test_env();

        // Create a test bookmark
        let mut tags = HashSet::new();
        tags.insert(Tag::new("test").unwrap());

        let bookmark = Bookmark::new(
            "https://example.com",
            "Example Site",
            "This is a description",
            tags,
            &crate::infrastructure::embeddings::DummyEmbedding,
        )
        .unwrap();

        use crate::infrastructure::interpolation::minijinja_engine::{
            MiniJinjaEngine, SafeShellExecutor,
        };
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let service = TemplateServiceImpl::with_editor("vim".to_string(), engine);

        // Edit the bookmark
        let (_result, edited) = service.edit_bookmark_with_template(Some(bookmark)).unwrap();

        // Verify the changes
        assert!(edited, "Should detect file was modified");
    }
}
