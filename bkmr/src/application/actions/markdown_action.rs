// src/application/actions/markdown_action.rs
use crate::app_state::AppState;
use crate::application::services::interpolation::InterpolationService;
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::repository::BookmarkRepository;
use crate::infrastructure::embeddings::DummyEmbedding;
use crate::util::helper::calc_content_hash;
use crate::util::path::{abspath, is_file_path};
use markdown::{to_html_with_options, Options};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

#[derive(Debug)]
pub struct MarkdownAction {
    interpolation_service: Arc<dyn InterpolationService>,
    repository: Option<Arc<dyn BookmarkRepository>>,
}

impl MarkdownAction {
    #[allow(dead_code)]
    pub fn new(interpolation_service: Arc<dyn InterpolationService>) -> Self {
        Self {
            interpolation_service,
            repository: None,
        }
    }

    // Constructor with repository for embedding support
    pub fn new_with_repository(
        interpolation_service: Arc<dyn InterpolationService>,
        repository: Arc<dyn BookmarkRepository>,
    ) -> Self {
        Self {
            interpolation_service,
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
                self.interpolation_service
                    .render_bookmark_url(bookmark)
                    .map_err(|e| DomainError::Other(format!("Failed to render markdown: {}", e)))?
            } else {
                markdown_content.clone()
            };

        // Update embedding if possible
        if let Err(e) = self.update_embedding(bookmark, &markdown_content) {
            error!("Failed to update embedding: {}", e);
            // Continue with rendering - don't fail the whole operation if embedding fails
        }

        debug!("Rendering markdown content to HTML");

        // Configure markdown options to properly handle tables and other features
        // let options = Options::default(); // CommonMark
        let options = Options::gfm(); // GitHub Flavored Markdown

        // Convert markdown to HTML with enhanced options
        let html_content = to_html_with_options(&rendered_markdown, &options)
            .map_err(|e| DomainError::Other(format!("Failed to render markdown: {}", e)))?;

        // Wrap the HTML content in a proper HTML document with enhanced styling
        let full_html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{}</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        :root {{
            --background-color: #ffffff;
            --text-color: #333333;
            --code-background: #f5f5f5;
            --blockquote-border: #ddd;
            --blockquote-color: #666;
            --link-color: #0366d6;
            --table-border: #ddd;
            --table-header-bg: #f2f2f2;
            --table-row-alt-bg: #f9f9f9;
            --base-font-size: 16px;
            --code-font-size: 0.99;
        }}

        @media (prefers-color-scheme: dark) {{
            :root {{
                --background-color: #1e1e1e;
                --text-color: #e0e0e0;
                --code-background: #2d2d2d;
                --blockquote-border: #555;
                --blockquote-color: #aaa;
                --link-color: #58a6ff;
                --table-border: #444;
                --table-header-bg: #2d2d2d;
                --table-row-alt-bg: #262626;
            }}
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            line-height: 1.6;
            color: var(--text-color);
            background-color: var(--background-color);
            max-width: 900px;
            margin: 0 auto;
            padding: 20px;
            font-size: var(--base-font-size);
        }}

        h1, h2, h3, h4, h5, h6 {{
            margin-top: 24px;
            margin-bottom: 16px;
            font-weight: 600;
            line-height: 1.25;
        }}

        h1 {{ font-size: 2em; border-bottom: 1px solid var(--table-border); padding-bottom: 0.3em; }}
        h2 {{ font-size: 1.5em; border-bottom: 1px solid var(--table-border); padding-bottom: 0.3em; }}

        a {{ color: var(--link-color); text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}

        /* Enhanced pre/code styling with syntax highlighting support */
        pre {{
            background-color: var(--code-background);
            padding: 16px;
            border-radius: 6px;
            overflow-x: auto;
            margin: 16px 0;
            font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
            font-size: var(--code-font-size);
            line-height: 1.45;
        }}

        code {{
            font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace;
            background-color: var(--code-background);
            padding: 0.2em 0.4em;
            border-radius: 3px;
            font-size: var(--code-font-size);
        }}

        /* Inline code should be slightly larger for better readability */
        p code, li code, td code {{
            font-size: calc(var(--code-font-size) * 1.05);
        }}

        pre code {{
            padding: 0;
            background-color: transparent;
            white-space: pre;
            word-break: normal;
            overflow-wrap: normal;
        }}

        /* Syntax highlighting classes */
        .hljs-keyword {{ color: #cf222e; }}
        .hljs-built_in {{ color: #e36209; }}
        .hljs-type {{ color: #953800; }}
        .hljs-literal {{ color: #0550ae; }}
        .hljs-number {{ color: #0550ae; }}
        .hljs-string {{ color: #0a3069; }}
        .hljs-comment {{ color: #6e7781; }}
        .hljs-doctag {{ color: #0550ae; }}
        .hljs-meta {{ color: #8250df; }}
        .hljs-function {{ color: #8250df; }}

        @media (prefers-color-scheme: dark) {{
            .hljs-keyword {{ color: #ff7b72; }}
            .hljs-built_in {{ color: #ffa657; }}
            .hljs-type {{ color: #ff7b72; }}
            .hljs-literal {{ color: #79c0ff; }}
            .hljs-number {{ color: #79c0ff; }}
            .hljs-string {{ color: #a5d6ff; }}
            .hljs-comment {{ color: #8b949e; }}
            .hljs-doctag {{ color: #79c0ff; }}
            .hljs-meta {{ color: #d2a8ff; }}
            .hljs-function {{ color: #d2a8ff; }}
        }}

        /* Enhanced blockquote styling */
        blockquote {{
            margin: 0;
            padding-left: 16px;
            padding-right: 16px;
            padding-bottom: 1px;
            padding-top: 1px;
            background: rgba(0, 0, 0, 0.05);
            border-left: 4px solid var(--blockquote-border);
            color: var(--blockquote-color);
            margin-bottom: 16px;
        }}

        @media (prefers-color-scheme: dark) {{
            blockquote {{
                background: rgba(255, 255, 255, 0.05);
            }}
        }}

        /* Enhanced image styling */
        img {{
            max-width: 100%;
            box-sizing: border-box;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            margin: 10px 0;
            border-radius: 4px;
        }}

        /* Enhanced table styling with explicit font size control */
        table {{
            border-collapse: collapse;
            width: 100%;
            margin-bottom: 16px;
            overflow: auto;
            font-size: 1em; /* Match body font size */
        }}

        th, td {{
            border: 1px solid var(--table-border);
            padding: 8px 13px;
            text-align: left;
            font-size: 1em; /* Consistent font size */
            vertical-align: top;
        }}

        th {{
            background-color: var(--table-header-bg);
            font-weight: 600;
        }}

        tr {{
            font-size: 1em; /* Ensure rows maintain consistent font size */
        }}

        tr:nth-child(even) {{
            background-color: var(--table-row-alt-bg);
        }}

        /* Lists styling */
        ul, ol {{
            padding-left: 2em;
            margin-top: 0;
            margin-bottom: 16px;
        }}

        li {{
            margin-top: 0.25em;
        }}

        /* Task lists */
        ul.contains-task-list {{
            list-style-type: none;
            padding-left: 1em;
        }}

        .task-list-item {{
            position: relative;
            padding-left: 1.5em;
        }}

        .task-list-item input {{
            position: absolute;
            left: 0;
            top: 0.25em;
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
    <!-- Highlight.js for code syntax highlighting -->
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/styles/github.min.css">
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/highlight.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/rust.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/java.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/python.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/bash.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/javascript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/typescript.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/json.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/cpp.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/yaml.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.7.0/languages/sql.min.js"></script>
    <script>
        document.addEventListener('DOMContentLoaded', (event) => {{
            document.querySelectorAll('pre code').forEach((block) => {{
                hljs.highlightBlock(block);
            }});

            // Add checkbox functionality for task lists
            document.querySelectorAll('.task-list-item input[type="checkbox"]').forEach(checkbox => {{
                checkbox.disabled = false;
                checkbox.addEventListener('change', function() {{
                    this.parentElement.classList.toggle('completed');
                }});
            }});
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
    use crate::application::services::interpolation::InterpolationServiceImpl;
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
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = MarkdownAction::new(interpolation_service);

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
        let interpolation_service =
            Arc::new(InterpolationServiceImpl::new(interpolation_engine.clone()));

        // Action without repository
        let action_no_repo = MarkdownAction::new(interpolation_service.clone());

        // Action with repository
        let repository = Arc::new(crate::util::testing::setup_test_db());
        let action_with_repo =
            MarkdownAction::new_with_repository(interpolation_service, repository);

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
            created_at: Some(chrono::Utc::now()),
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
            created_at: Some(chrono::Utc::now()),
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
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = MarkdownAction::new(interpolation_service);

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
            created_at: Some(chrono::Utc::now()),
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

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn test_execute_with_markdown_table() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = MarkdownAction::new(interpolation_service);

        // Create a test bookmark with markdown table content
        let markdown = "# Test Table\n\n| Column 1 | Column 2 | Column 3 |\n| -------- | -------- | -------- |\n| Cell 1   | Cell 2   | Cell 3   |\n| Cell 4   | Cell 5   | Cell 6   |";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Test Table Document".to_string(),
            description: "A test markdown document with tables".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
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

    #[test]
    #[ignore = "This test opens a browser which might not be available in CI"]
    fn test_execute_with_code_highlighting() {
        // Setup
        let _ = init_test_env();
        let _guard = EnvGuard::new();

        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let interpolation_service = Arc::new(InterpolationServiceImpl::new(interpolation_engine));
        let action = MarkdownAction::new(interpolation_service);

        // Create a test bookmark with code blocks
        let markdown = "# Code Highlighting\n\n```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```\n\n```python\ndef hello():\n    print(\"Hello, world!\")\n```";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Code Highlighting Document".to_string(),
            description: "A test markdown document with code blocks".to_string(),
            tags,
            access_count: 0,
            created_at: Some(chrono::Utc::now()),
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
