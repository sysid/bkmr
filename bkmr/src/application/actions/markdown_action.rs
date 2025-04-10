// src/application/actions/markdown_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::interpolation::interface::InterpolationEngine;
use markdown::to_html;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::Builder;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct MarkdownAction {
    interpolation_engine: Arc<dyn InterpolationEngine>,
}

impl MarkdownAction {
    pub fn new(interpolation_engine: Arc<dyn InterpolationEngine>) -> Self {
        Self { interpolation_engine }
    }
}

impl BookmarkAction for MarkdownAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        // Get the markdown content (stored in URL field)
        let markdown_content = &bookmark.url;

        // Apply any interpolation if the markdown contains template variables
        let rendered_markdown = if markdown_content.contains("{{") || markdown_content.contains("{%") {
            self.interpolation_engine.render_bookmark_url(bookmark)?
        } else {
            markdown_content.to_string()
        };

        debug!("Rendering markdown content to HTML");

        // Convert markdown to HTML
        let html_content = to_html(&rendered_markdown);

        // Wrap the HTML content in a proper HTML document with basic styling
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
</head>
<body>
    {}
</body>
</html>"#,
            bookmark.title,
            html_content
        );

        // Create a temporary HTML file with explicit .html extension
        let temp_dir = tempfile::Builder::new().prefix("bkmr-markdown-").tempdir()
            .map_err(|e| DomainError::Other(format!("Failed to create temporary directory: {}", e)))?;

        // Create a file path with .html extension
        let safe_title = bookmark.title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
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
    use crate::infrastructure::interpolation::minijinja_engine::{MiniJinjaEngine, SafeShellExecutor};
    use std::collections::HashSet;

    #[test]
    #[ignore = "Ignoring this test as it requires a browser to open"]
    fn test_markdown_action_renders_html() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = MarkdownAction::new(interpolation_engine);

        // Create a simple markdown document
        let markdown = "# Test Heading\n\nThis is a **test** of _markdown_ rendering.";
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
            embeddable: false,
        };

        // Act - This is tricky to test fully as it opens a browser,
        // so we just check that it doesn't throw an error
        let result = action.execute(&bookmark);

        // In a real test environment, we might want to mock the browser opening
        // and check the HTML content directly
        match result {
            Ok(_) => {}  // This is fine for this test
            Err(e) => {
                // If we're in a CI environment without a browser, this might fail
                // So we just check if it's an expected error
                if let DomainError::Other(msg) = &e {
                    if !msg.contains("Failed to open HTML in browser") {
                        panic!("Unexpected error: {}", e);
                    }
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }
    }

    #[test]
    #[ignore = "Ignoring this test as it requires a browser to open"]
    fn test_markdown_action_with_interpolation() {
        // Arrange
        let shell_executor = Arc::new(SafeShellExecutor::new());
        let interpolation_engine = Arc::new(MiniJinjaEngine::new(shell_executor));
        let action = MarkdownAction::new(interpolation_engine);

        // Create a markdown document with interpolation
        let markdown = "# Report for {{ current_date | strftime(\"%Y-%m-%d\") }}\n\nThis is a **test** with _interpolation_.";
        let mut tags = HashSet::new();
        tags.insert(Tag::new("_md_").unwrap());

        let bookmark = Bookmark {
            id: Some(1),
            url: markdown.to_string(),
            title: "Test Markdown with Interpolation".to_string(),
            description: "A test markdown document with interpolation".to_string(),
            tags,
            access_count: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            embedding: None,
            content_hash: None,
            embeddable: false,
        };

        // Act - This is tricky to test fully as it opens a browser
        let result = action.execute(&bookmark);

        // Same approach as the previous test
        match result {
            Ok(_) => {}
            Err(e) => {
                if let DomainError::Other(msg) = &e {
                    if !msg.contains("Failed to open HTML in browser") {
                        panic!("Unexpected error: {}", e);
                    }
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }
    }
}