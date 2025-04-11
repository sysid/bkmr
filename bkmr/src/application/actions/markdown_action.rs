// src/application/actions/markdown_action.rs
use crate::app_state::AppState;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use crate::domain::repositories::repository::BookmarkRepository;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::util::helper::calc_content_hash;
use crate::util::path::{abspath, is_file_path};
use markdown::to_html;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

#[derive(Debug)]
pub struct MarkdownAction {
    interpolation_engine: Arc<dyn InterpolationEngine>,
    repository: Option<Arc<dyn BookmarkRepository>>,
}

impl MarkdownAction {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self {
            interpolation_engine,
            repository: None,
        }
    }

    // Constructor with repository for embedding support
    pub fn new_with_repository(
        interpolation_engine: Arc<dyn InterpolationEngine>,
        repository: Arc<dyn BookmarkRepository>,
    ) -> Self {
        Self {
            interpolation_engine,
            repository: Some(repository),
        }
    }

    /// Reads markdown content from a file path
    fn read_markdown_from_file(&self, path_str: &str) -> DomainResult<String> {
        debug!("Attempting to read from path: {}", path_str);

        // First try to resolve the path with abspath (handles shell variables, ~, etc.)
        let resolved_path = match abspath(path_str) {
            Some(path) => {
                debug!("Path resolved with abspath: {}", path);
                path
            }
            None => {
                // If abspath fails, try as a relative path
                debug!("abspath failed, trying as relative path");
                let path = Path::new(path_str);
                if path.exists() {
                    path.to_string_lossy().to_string()
                } else {
                    return Err(DomainError::Other(format!(
                        "File not found: {}. Neither absolute nor relative path exists.",
                        path_str
                    )));
                }
            }
        };

        debug!("Reading from resolved path: {}", resolved_path);

        // Read the file contents
        fs::read_to_string(&resolved_path).map_err(|e| {
            DomainError::Other(format!(
                "Failed to read markdown file '{}': {}",
                resolved_path, e
            ))
        })
    }

    /// Check if embedding is allowed and possible
    fn can_update_embedding(&self, bookmark: &Bookmark) -> bool {
        // Check if we have a repository
        if self.repository.is_none() {
            return false;
        }

        // Check if the bookmark is embeddable
        if !bookmark.embeddable {
            return false;
        }

        // Check if OpenAI embeddings are enabled (not using DummyEmbedding)
        let app_state = AppState::read_global();
        app_state.context.embedder.as_any().type_id() != std::any::TypeId::of::<DummyEmbedding>()
    }

    /// Update bookmark with embedding if repository is available and conditions are met
    fn update_embedding(&self, bookmark: &Bookmark, content: &str) -> DomainResult<()> {
        // Check if embedding is allowed
        if !self.can_update_embedding(bookmark) {
            debug!("Embedding update skipped: not allowed or not possible");
            return Ok(());
        }

        let repository = self.repository.as_ref().unwrap();

        if let Some(id) = bookmark.id {
            // Get the current state of the bookmark
            let mut updated_bookmark = repository
                .get_by_id(id)?
                .ok_or_else(|| DomainError::BookmarkNotFound(id.to_string()))?;

            // Calculate content hash for the current content
            let content_hash = calc_content_hash(content);

            // Only update if content has changed
            if updated_bookmark.content_hash.as_ref() != Some(&content_hash) {
                debug!("Content changed, updating embedding for bookmark ID {}", id);

                // Get the app state for embedder
                let app_state = AppState::read_global();
                let embedder = &*app_state.context.embedder;

                // Generate embedding
                if let Some(embedding) = embedder.embed(content)? {
                    // Serialize the embedding
                    let serialized = crate::domain::embedding::serialize_embedding(embedding)?;

                    // Update the bookmark
                    updated_bookmark.embedding = Some(serialized);
                    updated_bookmark.content_hash = Some(content_hash);

                    // Save to repository
                    repository.update(&updated_bookmark)?;
                    info!("Successfully updated embedding for bookmark ID {}", id);
                }
            } else {
                debug!(
                    "Content unchanged, not updating embedding for bookmark ID {}",
                    id
                );
            }
        }

        Ok(())
    }
}

impl BookmarkAction for MarkdownAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the content from bookmark URL field
        let content_or_path = &bookmark.url;

        // Determine if the content is a file path or direct markdown
        let markdown_content = if is_file_path(content_or_path) {
            debug!("Treating content as a file path: {}", content_or_path);
            self.read_markdown_from_file(content_or_path)?
        } else {
            debug!("Treating content as direct markdown");
            content_or_path.to_string()
        };

        // Apply any interpolation if the markdown contains template variables
        let rendered_markdown =
            if markdown_content.contains("{{") || markdown_content.contains("{%") {
                self.interpolation_engine.render_bookmark_url(bookmark)?
            } else {
                markdown_content.clone()
            };

        // Update embedding if possible
        if let Err(e) = self.update_embedding(bookmark, &markdown_content) {
            error!("Failed to update embedding: {}", e);
            // Continue with rendering - don't fail the whole operation if embedding fails
        }

        debug!("Rendering markdown content to HTML");

        // Convert markdown to HTML
        let html_content = to_html(&rendered_markdown);

        // Wrap the HTML content in a proper HTML document with basic styling
        // Include MathJax for LaTeX rendering
        let full_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <meta charset="UTF-8">
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }}
        pre {{
            background-color: #f5f5f5;
            padding: 10px;
            border-radius: 4px;
            overflow-x: auto;
        }}
        code {{
            font-family: Consolas, Monaco, 'Andale Mono', monospace;
            background-color: #f5f5f5;
            padding: 2px 4px;
            border-radius: 3px;
        }}
        blockquote {{
            margin: 0;
            padding-left: 15px;
            border-left: 4px solid #ddd;
            color: #666;
        }}
        img {{
            max-width: 100%;
        }}
        table {{
            border-collapse: collapse;
            width: 100%;
        }}
        th, td {{
            border: 1px solid #ddd;
            padding: 8px;
        }}
        th {{
            background-color: #f2f2f2;
        }}
    </style>
    <!-- MathJax for LaTeX rendering -->
    <script src="https://cdnjs.cloudflare.com/ajax/libs/mathjax/2.7.7/MathJax.js?config=TeX-MML-AM_CHTML"></script>
    <script type="text/x-mathjax-config">
        MathJax.Hub.Config({{
            tex2jax: {{
                inlineMath: [['$','$'], ['\\(','\\)']],
                displayMath: [['$$','$$'], ['\\[','\\]']],
                processEscapes: true
            }}
        }});
    </script>
</head>
<body>
    {}
</body>
</html>"#,
            bookmark.title, html_content
        );

        // Create a temporary HTML file with explicit .html extension
        let temp_dir = tempfile::Builder::new()
            .prefix("bkmr-markdown-")
            .tempdir()
            .map_err(|e| {
                DomainError::Other(format!("Failed to create temporary directory: {}", e))
            })?;

        // Create a file path with .html extension
        let safe_title = bookmark
            .title
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let file_name = format!("{}.html", safe_title);
        let file_path = temp_dir.path().join(file_name);

        // Create and write to the file
        let mut file = File::create(&file_path)
            .map_err(|e| DomainError::Other(format!("Failed to create HTML file: {}", e)))?;

        file.write_all(full_html.as_bytes())
            .map_err(|e| DomainError::Other(format!("Failed to write HTML to file: {}", e)))?;

        debug!("Opening HTML file in browser: {:?}", file_path);

        // Open the HTML file in the default browser
        open::that(&file_path)
            .map_err(|e| DomainError::Other(format!("Failed to open HTML in browser: {}", e)))?;

        // Keep the temporary directory around until the program exits
        // This prevents the file from being deleted while the browser is using it
        std::mem::forget(temp_dir);

        Ok(())
    }

    fn description(&self) -> &'static str {
        "Render markdown and open in browser"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;
    use crate::infrastructure::interpolation::minijinja_engine::{
        MiniJinjaEngine, SafeShellExecutor,
    };
    use crate::util::testing::{init_test_env, EnvGuard};
    use std::collections::HashSet;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Test reading markdown from a file
    #[test]
    fn test_read_markdown_from_file() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = MarkdownAction::new(interpolation_engine);

        // Create a temporary markdown file
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_content = "# Test Markdown\n\nThis is a test.";
        write!(temp_file, "{}", test_content).unwrap();

        // Test reading the file
        let result = action.read_markdown_from_file(temp_file.path().to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content);

        // Test with non-existent file
        let result = action.read_markdown_from_file("/this/file/does/not/exist.md");
        assert!(result.is_err());
    }

    // Test embedding eligibility check
    #[test]
    fn test_can_update_embedding() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));

        // Action without repository
        let action_no_repo = MarkdownAction::new(interpolation_engine.clone());

        // Action with repository
        let repository = Arc::new(crate::util::testing::setup_test_db());
        let action_with_repo =
            MarkdownAction::new_with_repository(interpolation_engine, repository);

        // Create test bookmarks
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        // Bookmark with embeddable=true
        let embeddable_bookmark = Bookmark {
            id: Some(1),
            url: "# Test".to_string(),
            title: "Test Document".to_string(),
            description: "A test document".to_string(),
            tags: tags.clone(),
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
        };

        // Bookmark with embeddable=false
        let non_embeddable_bookmark = Bookmark {
            id: Some(2),
            url: "# Test".to_string(),
            title: "Test Document".to_string(),
            description: "A test document".to_string(),
            tags,
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        // Test cases
        assert!(
            !action_no_repo.can_update_embedding(&embeddable_bookmark),
            "Should return false when no repository is available"
        );

        // The DummyEmbedding is the default in test environment
        assert!(
            !action_with_repo.can_update_embedding(&non_embeddable_bookmark),
            "Should return false when bookmark is not embeddable"
        );

        // This would be true with OpenAI embeddings, but we can't easily test that
        // So we just verify that the embeddable flag is checked
    }

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn test_execute_with_direct_markdown() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = MarkdownAction::new(interpolation_engine);

        // Create a test bookmark with direct markdown content
        let markdown = "# Test Markdown\n\nThis is a **test** with math: $$E = mc^2$$";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Test Markdown Document".to_string(),
            description: "A test markdown document".to_string(),
            tags,
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: true,
        };

        // Execute the action
        let result = action.execute(&bookmark);

        // In a proper environment this should succeed
        // In CI it might fail due to no browser
        if result.is_err() {
            if let DomainError::Other(msg) = &result.unwrap_err() {
                // Only consider the test failed if it's not related to browser opening
                if !msg.contains("Failed to open HTML in browser") {
                    panic!("Test failed with unexpected error: {}", msg);
                }
            }
        }
    }
}
