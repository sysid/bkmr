use crate::domain::error::{DomainError, DomainResult};
use std::sync::Arc;
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, InsertTextFormat,
    TextEdit,
};
use tracing::{debug, instrument};

use crate::lsp::backend::BkmrConfig;
use crate::lsp::domain::{CompletionContext, Snippet, SnippetFilter};
use crate::lsp::services::{AsyncSnippetService, LanguageTranslator};

/// Service for handling completion logic
pub struct CompletionService {
    snippet_service: Arc<dyn AsyncSnippetService>,
    config: BkmrConfig,
}

impl std::fmt::Debug for CompletionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionService")
            .field("snippet_service", &"<AsyncSnippetService>")
            .field("config", &self.config)
            .finish()
    }
}

impl CompletionService {
    pub fn new(snippet_service: Arc<dyn AsyncSnippetService>) -> Self {
        Self::with_config(snippet_service, BkmrConfig::default())
    }

    pub fn with_config(snippet_service: Arc<dyn AsyncSnippetService>, config: BkmrConfig) -> Self {
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
            self.config.max_completions,
            self.config.enable_interpolation,
        )
    }

    /// Convert snippet to LSP completion item with proper text replacement
    pub fn snippet_to_completion_item(
        &self,
        snippet: &Snippet,
        query: &str,
        replacement_range: Option<tower_lsp_server::ls_types::Range>,
        language_id: &str,
        uri: &tower_lsp_server::ls_types::Uri,
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
            documentation: Some(Documentation::String(truncate_preview(
                &snippet_content,
                500,
            ))),
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

/// Truncate content to at most `max_chars` characters for the documentation
/// preview, respecting char boundaries (byte-slicing panics on multibyte UTF-8).
fn truncate_preview(content: &str, max_chars: usize) -> String {
    match content.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => format!("{}...", &content[..byte_idx]),
        None => content.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::testing::{init_test_env, EnvGuard};
    use tower_lsp_server::ls_types::{Position, Range, Uri};

    // Tests run single-threaded (--test-threads=1) against a shared SQLite file.
    // Always construct services via TestServiceContainer, never production factories.

    #[tokio::test]
    async fn given_context_with_query_when_getting_completions_then_returns_filtered_items() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let lsp_bundle = ctx.create_lsp_services();
        let service = lsp_bundle.completion_service;

        let uri = "file:///test.rs".parse::<Uri>().expect("parse URI");
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
    async fn given_query_prefix_with_hyphen_when_getting_completions_then_does_not_error() {
        // Regression: a word ending in a hyphen produced the FTS term `metadata:foo-*`,
        // which SQLite FTS5 rejects with `syntax error near "*"`, failing the whole
        // combined query and returning zero completions.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let service = ctx.create_lsp_services().completion_service;

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
        let context = CompletionContext::new(
            "file:///test.mk".parse::<Uri>().expect("parse URI"),
            Position {
                line: 0,
                character: 4,
            },
            Some("make".to_string()),
        )
        .with_query(crate::lsp::domain::CompletionQuery::new(
            "foo-".to_string(),
            range,
        ));

        let result = service.get_completions(&context).await;

        assert!(
            result.is_ok(),
            "hyphenated prefix must not crash FTS: {:?}",
            result.err()
        );
    }

    fn completion_context_for(language_id: &str) -> CompletionContext {
        CompletionContext::new(
            "file:///test.src".parse::<Uri>().expect("parse URI"),
            Position {
                line: 0,
                character: 0,
            },
            Some(language_id.to_string()),
        )
    }

    #[tokio::test]
    async fn given_hyphenated_language_id_when_getting_completions_then_returns_matching_snippet() {
        // Regression: the language id was spliced unquoted into the FTS query, and
        // `tags:objective-c` is an FTS5 syntax error ("no such column: c"), so every
        // completion request in such a buffer failed.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let services = ctx.create_lsp_services();
        services
            .command_service
            .create_snippet(
                "[[Foo alloc] init]",
                "ObjC Alloc Init",
                None,
                vec!["objective-c".to_string()],
            )
            .expect("create snippet");

        let items = services
            .completion_service
            .get_completions(&completion_context_for("objective-c"))
            .await
            .expect("hyphenated language id must not break completion");

        assert!(items.iter().any(|i| i.label == "ObjC Alloc Init"));
    }

    #[tokio::test]
    async fn given_bookmark_tagged_snip_without_system_tag_when_getting_completions_then_excluded()
    {
        // Regression: FTS tokenizes `_snip_` to `snip`, so a plain bookmark tagged
        // `snip` matched the snippet filter and leaked into completion.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let tags: std::collections::HashSet<crate::domain::tag::Tag> = ["snip", "rust"]
            .iter()
            .map(|t| crate::domain::tag::Tag::new(t).expect("tag"))
            .collect();
        ctx.bookmark_service()
            .add_bookmark(
                "https://example.com/not-a-snippet",
                Some("Not A Snippet"),
                None,
                Some(&tags),
                false,
                false,
                None,
            )
            .expect("add bookmark");
        let services = ctx.create_lsp_services();

        let items = services
            .completion_service
            .get_completions(&completion_context_for("rust"))
            .await
            .expect("completions");

        assert!(!items.iter().any(|i| i.label == "Not A Snippet"));
    }

    #[tokio::test]
    async fn given_snippet_tagged_with_hyphenated_superstring_when_getting_completions_then_excluded(
    ) {
        // Regression: FTS tokenizes `rust-analyzer` to `rust` + `analyzer`, so the
        // tag `rust-analyzer` matched a `rust` buffer.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let services = ctx.create_lsp_services();
        services
            .command_service
            .create_snippet(
                "\"rust-analyzer.check.command\": \"clippy\"",
                "RA Clippy Setting",
                None,
                vec!["rust-analyzer".to_string()],
            )
            .expect("create snippet");

        let items = services
            .completion_service
            .get_completions(&completion_context_for("rust"))
            .await
            .expect("completions");

        assert!(!items.iter().any(|i| i.label == "RA Clippy Setting"));
    }

    #[tokio::test]
    async fn given_snippet_tagged_with_language_alias_when_getting_completions_then_included() {
        // A snippet tagged `js` must show in a buffer whose languageId is `javascript`.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let services = ctx.create_lsp_services();
        services
            .command_service
            .create_snippet(
                "console.log($1)",
                "Alias Console Log",
                None,
                vec!["js".to_string()],
            )
            .expect("create snippet");

        let items = services
            .completion_service
            .get_completions(&completion_context_for("javascript"))
            .await
            .expect("completions");

        assert!(items.iter().any(|i| i.label == "Alias Console Log"));
    }

    #[tokio::test]
    async fn given_universal_snippet_when_getting_completions_for_any_language_then_included() {
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let ctx = crate::util::test_context::TestContext::new();
        let services = ctx.create_lsp_services();
        services
            .command_service
            .create_snippet(
                "// TODO: $1",
                "Universal Todo",
                None,
                vec!["universal".to_string()],
            )
            .expect("create snippet");

        let items = services
            .completion_service
            .get_completions(&completion_context_for("python"))
            .await
            .expect("completions");

        assert!(items.iter().any(|i| i.label == "Universal Todo"));
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

        let uri = "file:///test.rs".parse::<Uri>().expect("parse URI");

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

        let uri = "file:///test.rs".parse::<Uri>().expect("parse URI");

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

        let uri = "file:///test.py".parse::<Uri>().expect("parse URI");

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

        let uri = "file:///test.rs".parse::<Uri>().expect("parse URI");
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
    async fn given_multibyte_content_longer_than_preview_when_creating_item_then_does_not_panic() {
        // Documentation preview truncation must respect char boundaries:
        // 200 x '€' (3 bytes each) = 600 bytes, and byte 500 falls mid-char.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let snippet = Snippet::new(
            1,
            "Multibyte".to_string(),
            "€".repeat(200),
            "desc".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        let ctx = crate::util::test_context::TestContext::new();
        let service = ctx.create_lsp_services().completion_service;
        let uri = "file:///test.rs".parse::<Uri>().expect("parse URI");

        let result = service.snippet_to_completion_item(&snippet, "", None, "rust", &uri);

        assert!(result.is_ok(), "must not panic or fail: {:?}", result.err());
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
