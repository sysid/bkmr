//! Command service for handling LSP commands
//!
//! Provides functionality for executing LSP commands including snippet CRUD operations.

use crate::application::services::BookmarkService;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::repositories::query::{BookmarkQuery, SortDirection};
use crate::domain::tag::Tag;
use crate::lsp::domain::LanguageRegistry;
use crate::lsp::error::{LspError, LspResult};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};
use tracing::{debug, instrument};

/// Service for handling LSP command execution
#[derive(Debug)]
pub struct CommandService {
    bookmark_service: Arc<dyn BookmarkService>,
}

impl CommandService {
    /// Create CommandService with dependency injection
    pub fn with_service(bookmark_service: Arc<dyn BookmarkService>) -> Self {
        Self {
            bookmark_service,
        }
    }

    /// Create a new snippet
    #[instrument(skip(self))]
    pub fn create_snippet(
        &self,
        url: &str,
        title: &str,
        description: Option<&str>,
        tags: Vec<String>,
    ) -> LspResult<Value> {
        debug!("Creating snippet: title={}, tags={:?}", title, tags);

        // Prepare tags with _snip_ system tag
        let mut tag_set = HashSet::new();
        tag_set.insert(Tag::new("_snip_").map_err(LspError::from)?);
        for tag in tags {
            tag_set.insert(Tag::new(&tag).map_err(LspError::from)?);
        }

        // Create bookmark
        let bookmark = self
            .bookmark_service
            .add_bookmark(
                url,
                Some(title),
                description,
                Some(&tag_set),
                false, // Don't fetch metadata for snippets
            )
            .map_err(LspError::from)?;

        Ok(Self::bookmark_to_snippet_json(&bookmark))
    }

    /// List snippets filtered by language
    #[instrument(skip(self))]
    pub fn list_snippets(&self, language_id: Option<&str>) -> LspResult<Value> {
        debug!("Listing snippets for language: {:?}", language_id);

        // Build query for snippets with language filter
        let mut query = BookmarkQuery::default();

        // Must have _snip_ tag
        let snip_tag = Tag::new("_snip_").map_err(LspError::from)?;
        let mut tags_all = HashSet::new();
        tags_all.insert(snip_tag);

        // If language specified, filter by language tag
        if let Some(lang) = language_id {
            // Map LSP language ID to our tag format
            let language_tag = Self::map_language_id_to_tag(lang);
            let lang_tag = Tag::new(&language_tag).map_err(LspError::from)?;
            tags_all.insert(lang_tag);
        }

        query.tags_all = Some(tags_all);
        query.sort_by_date = Some(SortDirection::Descending);

        let bookmarks = self
            .bookmark_service
            .search_bookmarks(&query)
            .map_err(LspError::from)?;

        let snippets: Vec<Value> = bookmarks
            .iter()
            .map(Self::bookmark_to_snippet_json)
            .collect();

        Ok(json!({
            "snippets": snippets
        }))
    }

    /// Get a single snippet by ID
    #[instrument(skip(self))]
    pub fn get_snippet(&self, id: i32) -> LspResult<Value> {
        debug!("Getting snippet with ID: {}", id);

        let bookmark = self
            .bookmark_service
            .get_bookmark(id)
            .map_err(LspError::from)?
            .ok_or_else(|| LspError::NotFound(format!("Snippet with ID {} not found", id)))?;

        // Verify it's a snippet
        if !bookmark.tags.iter().any(|t| t.value() == "_snip_") {
            return Err(LspError::InvalidInput(format!(
                "Bookmark {} is not a snippet",
                id
            )));
        }

        Ok(Self::bookmark_to_snippet_json(&bookmark))
    }

    /// Update an existing snippet
    #[instrument(skip(self))]
    pub fn update_snippet(
        &self,
        id: i32,
        url: Option<&str>,
        title: Option<&str>,
        description: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> LspResult<Value> {
        debug!(
            "Updating snippet {}: title={:?}, tags={:?}",
            id, title, tags
        );

        // Get existing bookmark
        let mut bookmark = self
            .bookmark_service
            .get_bookmark(id)
            .map_err(LspError::from)?
            .ok_or_else(|| LspError::NotFound(format!("Snippet with ID {} not found", id)))?;

        // Verify it's a snippet
        if !bookmark.tags.iter().any(|t| t.value() == "_snip_") {
            return Err(LspError::InvalidInput(format!(
                "Bookmark {} is not a snippet",
                id
            )));
        }

        // Update fields if provided
        if let Some(new_url) = url {
            bookmark.url = new_url.to_string();
        }
        if let Some(new_title) = title {
            bookmark.title = new_title.to_string();
        }
        if let Some(new_desc) = description {
            bookmark.description = new_desc.to_string();
        }

        // Update bookmark (without forcing embedding)
        let mut updated = self
            .bookmark_service
            .update_bookmark(bookmark, false)
            .map_err(LspError::from)?;

        // Handle tag updates separately if provided
        if let Some(new_tags) = tags {
            let mut tag_set = HashSet::new();
            // Always preserve _snip_ system tag
            tag_set.insert(Tag::new("_snip_").map_err(LspError::from)?);
            for tag in new_tags {
                tag_set.insert(Tag::new(&tag).map_err(LspError::from)?);
            }
            updated = self
                .bookmark_service
                .replace_bookmark_tags(id, &tag_set)
                .map_err(LspError::from)?;
        }

        Ok(Self::bookmark_to_snippet_json(&updated))
    }

    /// Delete a snippet
    #[instrument(skip(self))]
    pub fn delete_snippet(&self, id: i32) -> LspResult<Value> {
        debug!("Deleting snippet with ID: {}", id);

        // Get bookmark to verify it's a snippet
        let bookmark = self
            .bookmark_service
            .get_bookmark(id)
            .map_err(LspError::from)?
            .ok_or_else(|| LspError::NotFound(format!("Snippet with ID {} not found", id)))?;

        // Verify it's a snippet
        if !bookmark.tags.iter().any(|t| t.value() == "_snip_") {
            return Err(LspError::InvalidInput(format!(
                "Bookmark {} is not a snippet",
                id
            )));
        }

        let deleted = self
            .bookmark_service
            .delete_bookmark(id)
            .map_err(LspError::from)?;

        Ok(json!({
            "success": deleted,
            "id": id
        }))
    }

    /// Convert a Bookmark to snippet JSON representation
    fn bookmark_to_snippet_json(bookmark: &Bookmark) -> Value {
        json!({
            "id": bookmark.id,
            "url": bookmark.url,
            "title": bookmark.title,
            "description": bookmark.description,
            "tags": bookmark.tags.iter().map(|t| t.value()).collect::<Vec<_>>(),
        })
    }

    /// Map LSP language ID to our tag format
    fn map_language_id_to_tag(language_id: &str) -> String {
        match language_id {
            "rust" => "rust",
            "python" => "python",
            "javascript" | "javascriptreact" => "js",
            "typescript" | "typescriptreact" => "ts",
            "shellscript" | "bash" | "sh" => "sh",
            "go" => "go",
            "java" => "java",
            "cpp" | "c" => "cpp",
            "html" => "html",
            "css" | "scss" | "sass" => "css",
            "markdown" => "md",
            "yaml" => "yaml",
            "json" => "json",
            "sql" => "sql",
            "ruby" => "ruby",
            "php" => "php",
            _ => language_id,
        }
        .to_string()
    }
    /// Execute the insertFilepathComment command
    #[instrument(skip(file_uri))]
    pub fn insert_filepath_comment(file_uri: &str) -> DomainResult<WorkspaceEdit> {
        let relative_path = Self::get_relative_path(file_uri)?;
        let comment_syntax = LanguageRegistry::get_comment_syntax(file_uri);

        let comment_text = match comment_syntax {
            "<!--" => format!("<!-- {} -->\n", relative_path),
            "/*" => format!("/* {} */\n", relative_path),
            _ => format!("{} {}\n", comment_syntax, relative_path),
        };

        debug!("Inserting filepath comment: {}", comment_text.trim());

        // Create a text edit to insert at the beginning of the file
        let edit = TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            new_text: comment_text,
        };

        let uri = Url::parse(file_uri)
            .map_err(|e| DomainError::Other(format!("Parse file URI for workspace edit: {}", e)))?;

        let mut changes = HashMap::new();
        changes.insert(uri, vec![edit]);

        Ok(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    /// Get the relative path from project root
    fn get_relative_path(file_uri: &str) -> DomainResult<String> {
        let url = Url::parse(file_uri)
            .map_err(|e| DomainError::Other(format!("Parse file URI: {}", e)))?;

        let file_path = url
            .to_file_path()
            .map_err(|_| DomainError::Other("Convert URL to file path".to_string()))?;

        // Try to find a project root by looking for common indicators
        let mut current = file_path.as_path();
        while let Some(parent) = current.parent() {
            // Check for common project root indicators
            if parent.join("Cargo.toml").exists()
                || parent.join("package.json").exists()
                || parent.join("pom.xml").exists()
                || parent.join("build.gradle").exists()
                || parent.join("build.gradle.kts").exists()
                || parent.join("Makefile").exists()
                || parent.join(".git").exists()
            {
                // Found project root, return relative path
                if let Ok(rel_path) = file_path.strip_prefix(parent) {
                    return Ok(rel_path.to_string_lossy().to_string());
                }
                break;
            }
            current = parent;
        }

        // Fall back to just the filename if no project root found
        file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| DomainError::Other("Extract filename from file path".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::services::bookmark_service_impl::BookmarkServiceImpl;
    use crate::infrastructure::repositories::json_import_repository::JsonImportRepository;
    use crate::util::testing::{init_test_env, setup_test_db, EnvGuard};
    use std::sync::Arc;

    #[test]
    fn given_valid_snippet_data_when_creating_then_returns_snippet_with_id() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(JsonImportRepository::new()),
        ));
        let service = CommandService::with_service(bookmark_service);

        // Act
        let result = service.create_snippet(
            "fn example_test_snippet() { println!(\"Hello from test\"); }",
            "Example Test Function",
            Some("A simple example function"),
            vec!["rust".to_string(), "example".to_string()],
        );

        // Assert
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.get("id").is_some());
        assert_eq!(
            json.get("title").unwrap().as_str().unwrap(),
            "Example Test Function"
        );
        assert_eq!(
            json.get("url").unwrap().as_str().unwrap(),
            "fn example_test_snippet() { println!(\"Hello from test\"); }"
        );

        let tags = json.get("tags").unwrap().as_array().unwrap();
        assert!(tags.iter().any(|t| t.as_str() == Some("_snip_")));
        assert!(tags.iter().any(|t| t.as_str() == Some("rust")));
    }

    #[test]
    fn given_language_filter_when_listing_snippets_then_returns_filtered_results() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(JsonImportRepository::new()),
        ));
        let service = CommandService::with_service(bookmark_service);

        // Create snippets with different languages
        service
            .create_snippet(
                "print('Python snippet test')",
                "Python Print Test",
                None,
                vec!["python".to_string()],
            )
            .unwrap();
        service
            .create_snippet(
                "fn rust_test() { println!(\"test\"); }",
                "Rust Function Test",
                None,
                vec!["rust".to_string()],
            )
            .unwrap();

        // Act
        let result = service.list_snippets(Some("rust"));

        // Assert
        assert!(result.is_ok());
        let json = result.unwrap();
        let snippets = json.get("snippets").unwrap().as_array().unwrap();

        // Should only contain Rust snippets
        for snippet in snippets {
            let tags = snippet.get("tags").unwrap().as_array().unwrap();
            assert!(tags.iter().any(|t| t.as_str() == Some("rust")));
        }
    }

    #[test]
    fn given_existing_snippet_when_updating_then_preserves_system_tag() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(JsonImportRepository::new()),
        ));
        let service = CommandService::with_service(bookmark_service);

        // Create a snippet
        let created = service
            .create_snippet(
                "original test content for update",
                "Original Test Title",
                None,
                vec!["rust".to_string()],
            )
            .unwrap();
        let id = created.get("id").unwrap().as_i64().unwrap() as i32;

        // Act - Update with new tags
        let result = service.update_snippet(
            id,
            Some("updated test content for test"),
            Some("Updated Title"),
            Some("Updated description"),
            Some(vec!["python".to_string(), "updated".to_string()]),
        );

        // Assert
        assert!(result.is_ok());
        let updated = result.unwrap();

        assert_eq!(
            updated.get("url").unwrap().as_str().unwrap(),
            "updated test content for test"
        );
        assert_eq!(
            updated.get("title").unwrap().as_str().unwrap(),
            "Updated Title"
        );

        let tags = updated.get("tags").unwrap().as_array().unwrap();
        assert!(tags.iter().any(|t| t.as_str() == Some("_snip_"))); // System tag preserved
        assert!(tags.iter().any(|t| t.as_str() == Some("python")));
        assert!(tags.iter().any(|t| t.as_str() == Some("updated")));
        assert!(!tags.iter().any(|t| t.as_str() == Some("rust"))); // Old tag removed
    }

    #[test]
    fn given_non_snippet_bookmark_when_getting_as_snippet_then_returns_error() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(JsonImportRepository::new()),
        ));
        let service = CommandService::with_service(bookmark_service.clone());

        // Create a regular bookmark (not a snippet)
        use std::collections::HashSet;
        let mut tags = HashSet::new();
        tags.insert(Tag::new("website").unwrap());
        let bookmark = bookmark_service
            .add_bookmark(
                "https://example-test-non-snippet.com",
                Some("Example Test Site"),
                None,
                Some(&tags),
                false,
            )
            .unwrap();
        let id = bookmark.id.unwrap();

        // Act
        let result = service.get_snippet(id);

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            LspError::InvalidInput(msg) => assert!(msg.contains("not a snippet")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn given_non_existent_id_when_deleting_snippet_then_returns_not_found() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(JsonImportRepository::new()),
        ));
        let service = CommandService::with_service(bookmark_service);

        // Act
        let result = service.delete_snippet(99999);

        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            LspError::NotFound(msg) => assert!(msg.contains("99999")),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn given_rust_file_when_inserting_filepath_comment_then_uses_double_slash() {
        // Arrange
        let file_uri = "file:///path/to/test.rs";

        // Act
        let result = CommandService::insert_filepath_comment(file_uri);

        // Assert
        assert!(result.is_ok());
        let workspace_edit = result.expect("valid workspace edit");

        let changes = workspace_edit.changes.expect("workspace changes");
        let edits = changes.values().next().expect("text edits");
        let edit = &edits[0];

        assert!(edit.new_text.starts_with("// "));
        assert!(edit.new_text.contains("test.rs"));
    }

    #[test]
    fn given_html_file_when_inserting_filepath_comment_then_uses_html_comment() {
        // Arrange
        let file_uri = "file:///path/to/test.html";

        // Act
        let result = CommandService::insert_filepath_comment(file_uri);

        // Assert
        assert!(result.is_ok());
        let workspace_edit = result.expect("valid workspace edit");

        let changes = workspace_edit.changes.expect("workspace changes");
        let edits = changes.values().next().expect("text edits");
        let edit = &edits[0];

        assert!(edit.new_text.starts_with("<!-- "));
        assert!(edit.new_text.ends_with(" -->\n"));
        assert!(edit.new_text.contains("test.html"));
    }

    #[test]
    fn given_python_file_when_inserting_filepath_comment_then_uses_hash() {
        // Arrange
        let file_uri = "file:///path/to/test.py";

        // Act
        let result = CommandService::insert_filepath_comment(file_uri);

        // Assert
        assert!(result.is_ok());
        let workspace_edit = result.expect("valid workspace edit");

        let changes = workspace_edit.changes.expect("workspace changes");
        let edits = changes.values().next().expect("text edits");
        let edit = &edits[0];

        assert!(edit.new_text.starts_with("# "));
        assert!(edit.new_text.contains("test.py"));
    }

    #[test]
    fn given_invalid_uri_when_inserting_filepath_comment_then_returns_error() {
        // Arrange
        let file_uri = "invalid-uri";

        // Act
        let result = CommandService::insert_filepath_comment(file_uri);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn given_file_when_getting_relative_path_then_returns_filename_fallback() {
        // Arrange
        // This test would need a real project structure to work properly
        // For now, we'll test the fallback behavior
        let file_uri = "file:///some/deep/path/test.rs";

        // Act
        let result = CommandService::get_relative_path(file_uri);

        // Assert
        assert!(result.is_ok());
        let path = result.expect("valid relative path");
        assert_eq!(path, "test.rs"); // Should fall back to filename
    }
}
