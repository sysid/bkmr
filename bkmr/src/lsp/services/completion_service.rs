use crate::domain::error::{DomainError, DomainResult};
use std::sync::Arc;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, InsertTextFormat,
    TextEdit,
};
use tracing::{debug, instrument};

use crate::lsp::backend::BkmrConfig;
use crate::lsp::domain::{CompletionContext, Snippet, SnippetFilter};
use crate::lsp::services::{AsyncSnippetService, LanguageTranslator, LspSnippetService};

/// Service for handling completion logic
pub struct CompletionService {
    snippet_service: Arc<LspSnippetService>,
    config: BkmrConfig,
}

impl std::fmt::Debug for CompletionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionService")
            .field("snippet_service", &"<LspSnippetService>")
            .field("config", &self.config)
            .finish()
    }
}

impl CompletionService {
    pub fn new(snippet_service: Arc<LspSnippetService>) -> Self {
        Self::with_config(snippet_service, BkmrConfig::default())
    }

    pub fn with_config(snippet_service: Arc<LspSnippetService>, config: BkmrConfig) -> Self {
        Self {
            snippet_service,
            config,
        }
    }

    /// Generate completion items from context
    #[instrument(skip(self))]
    pub async fn get_completions(
        &self,
        context: &CompletionContext,
    ) -> DomainResult<Vec<CompletionItem>> {
        let filter = self.build_snippet_filter(context);

        let snippets = self.snippet_service.fetch_snippets(&filter).await?;

        let completion_items: Result<Vec<CompletionItem>, _> = snippets
            .iter()
            .map(|snippet| {
                self.snippet_to_completion_item(
                    snippet,
                    context.get_query_text().unwrap_or(""),
                    context.get_replacement_range(),
                    context.language_id.as_deref().unwrap_or("unknown"),
                    &context.uri,
                )
            })
            .collect();

        let completion_items = completion_items.map_err(|e| {
            DomainError::Other(format!(
                "Failed to convert snippets to completion items: {}",
                e
            ))
        })?;

        debug!("Generated {} completion items", completion_items.len());
        Ok(completion_items)
    }

    /// Build snippet filter from completion context
    fn build_snippet_filter(&self, context: &CompletionContext) -> SnippetFilter {
        let query_prefix = context.get_query_text().map(|s| s.to_string());
        SnippetFilter::new(
            context.language_id.clone(),
            query_prefix,
            50, // TODO: Make configurable
            self.config.enable_interpolation,
        )
    }

    /// Convert snippet to LSP completion item with proper text replacement
    pub fn snippet_to_completion_item(
        &self,
        snippet: &Snippet,
        query: &str,
        replacement_range: Option<tower_lsp::lsp_types::Range>,
        language_id: &str,
        uri: &tower_lsp::lsp_types::Url,
    ) -> DomainResult<CompletionItem> {
        // Snippet content is already processed (interpolated) by LspSnippetService
        // We only need to apply language translation
        let snippet_content = LanguageTranslator::translate_snippet(snippet, language_id, uri)?;

        let label = snippet.title.clone();

        debug!(
            "Creating completion item: query='{}', label='{}', content_preview='{}'",
            query,
            label,
            snippet_content.chars().take(20).collect::<String>()
        );

        // Determine if this should be treated as plain text
        let (item_kind, text_format, detail_text) = if snippet.is_plain() {
            (
                CompletionItemKind::TEXT,
                InsertTextFormat::PLAIN_TEXT,
                "bkmr plain text",
            )
        } else {
            (
                CompletionItemKind::SNIPPET,
                InsertTextFormat::SNIPPET,
                "bkmr snippet",
            )
        };

        let mut completion_item = CompletionItem {
            label: label.clone(),
            kind: Some(item_kind),
            detail: Some(detail_text.to_string()),
            documentation: Some(Documentation::String(if snippet_content.len() > 500 {
                format!("{}...", &snippet_content[..500])
            } else {
                snippet_content.clone()
            })),
            insert_text_format: Some(text_format),
            filter_text: Some(label.clone()),
            sort_text: Some(label.clone()),
            ..Default::default()
        };

        // Use TextEdit for proper replacement if we have a range
        if let Some(range) = replacement_range {
            completion_item.text_edit = Some(CompletionTextEdit::Edit(TextEdit {
                range,
                new_text: snippet_content,
            }));
            debug!("Set text_edit for range replacement: {:?}", range);
        } else {
            // Fallback to insert_text for backward compatibility
            completion_item.insert_text = Some(snippet_content);
            debug!("Using fallback insert_text (no range available)");
        }

        Ok(completion_item)
    }

    /// Health check for the completion service
    pub async fn health_check(&self) -> DomainResult<()> {
        self.snippet_service
            .health_check()
            .await
            .map_err(|e| DomainError::Other(e.to_string()))
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, EnvGuard};
    use tower_lsp::lsp_types::{Position, Range, Url};

    // TODO: consolidate this
    /*
     * IMPORTANT: LSP Test Database Synchronization Requirements
     *
     * All tests in this module that access the database must follow these patterns:
     *
     * 1. Tests run single-threaded (--test-threads=1) so no special synchronization needed
     * 2. NEVER use LspSnippetService::new() in tests - it calls factory methods that
     *    bypass test environment setup and try to access production database
     * 3. ALWAYS use proper test service construction pattern:
     *    - Call init_test_env(), EnvGuard::new(), setup_test_db()
     *    - Manually construct BookmarkServiceImpl with test repository
     *    - Use LspSnippetService::with_service() constructor
     *
     * This was discovered when make test-all was failing due to race conditions.
     * The issue was that LspSnippetService::new() -> factory::create_bookmark_service()
     * would try to read from global AppState and access a database that doesn't exist
     * in the test environment, causing "Database not found" errors.
     *
     * See CLAUDE.md for complete details on this synchronization issue.
     */

    #[tokio::test]
    async fn given_context_with_query_when_getting_completions_then_returns_filtered_items() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = Url::parse("file:///test.rs").expect("parse URI");
        let context = CompletionContext::new(
            uri,
            Position {
                line: 0,
                character: 5,
            },
            Some("rust".to_string()),
        );

        // Act
        let result = service.get_completions(&context).await;

        // Assert
        assert!(result.is_ok());
        let items = result.expect("valid completion items");
        // Note: Actual number depends on database content
        debug!("Got {} completion items", items.len());
    }

    #[tokio::test]
    async fn given_plain_snippet_when_creating_completion_item_then_uses_plain_text_format() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let plain_snippet = Snippet::new(
            1,
            "Plain Text".to_string(),
            "simple text content with no ${1:placeholders}".to_string(),
            "Plain text snippet".to_string(),
            vec!["plain".to_string(), "_snip_".to_string()],
        );

        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = Url::parse("file:///test.rs").expect("parse URI");

        // Act
        let result = service.snippet_to_completion_item(&plain_snippet, "", None, "rust", &uri);

        // Assert
        assert!(result.is_ok());
        let item = result.expect("valid completion item");

        assert_eq!(item.kind, Some(CompletionItemKind::TEXT));
        assert_eq!(item.insert_text_format, Some(InsertTextFormat::PLAIN_TEXT));
        assert_eq!(item.detail, Some("bkmr plain text".to_string()));
        assert_eq!(item.label, "Plain Text");
    }

    #[tokio::test]
    async fn given_regular_snippet_when_creating_completion_item_then_uses_snippet_format() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let regular_snippet = Snippet::new(
            1,
            "Regular Snippet".to_string(),
            "snippet with ${1:placeholder}".to_string(),
            "Regular snippet".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = Url::parse("file:///test.rs").expect("parse URI");

        // Act
        let result = service.snippet_to_completion_item(&regular_snippet, "", None, "rust", &uri);

        // Assert
        assert!(result.is_ok());
        let item = result.expect("valid completion item");

        assert_eq!(item.kind, Some(CompletionItemKind::SNIPPET));
        assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
        assert_eq!(item.detail, Some("bkmr snippet".to_string()));
        assert_eq!(item.label, "Regular Snippet");
    }

    #[tokio::test]
    async fn given_universal_snippet_when_creating_completion_item_then_translates_content() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let universal_snippet = Snippet::new(
            1,
            "Universal Comment".to_string(),
            "// This is a universal comment".to_string(),
            "Universal snippet".to_string(),
            vec!["universal".to_string(), "_snip_".to_string()],
        );

        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = Url::parse("file:///test.py").expect("parse URI");

        // Act
        let result =
            service.snippet_to_completion_item(&universal_snippet, "", None, "python", &uri);

        // Assert
        assert!(result.is_ok());
        let item = result.expect("valid completion item");

        // Should have translated Rust comment to Python comment
        let insert_text = item.insert_text.expect("insert text");
        assert!(insert_text.contains("# This is a universal comment"));
    }

    #[tokio::test]
    async fn given_completion_item_with_range_when_creating_then_uses_text_edit() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let snippet = Snippet::new(
            1,
            "Test Snippet".to_string(),
            "test content".to_string(),
            "Test description".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = Url::parse("file:///test.rs").expect("parse URI");
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 4,
            },
        };

        // Act
        let result =
            service.snippet_to_completion_item(&snippet, "test", Some(range), "rust", &uri);

        // Assert
        assert!(result.is_ok());
        let item = result.expect("valid completion item");

        match item.text_edit {
            Some(CompletionTextEdit::Edit(edit)) => {
                assert_eq!(edit.range, range);
                assert_eq!(edit.new_text, "test content");
            }
            _ => panic!("Expected text edit"),
        }
    }

    #[tokio::test]
    async fn given_healthy_service_when_health_check_then_returns_ok() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        // Act
        let result = service.health_check().await;

        // Assert
        assert!(result.is_ok());
    }
}
