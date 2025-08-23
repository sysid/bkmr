#[cfg(test)]
mod integration_tests {
    use crate::lsp::backend::BkmrConfig;
    use crate::lsp::domain::{CompletionContext, Snippet};
    use crate::lsp::services::{CompletionService, LspSnippetService};
    use crate::util::testing::{init_test_env, EnvGuard};
    use serial_test::serial;
    use std::sync::Arc;
    use tower_lsp::lsp_types::{Position, Url};

    /*
     * IMPORTANT: LSP Integration Test Database Synchronization
     * 
     * These integration tests require careful database access patterns:
     * 
     * 1. ALWAYS use #[serial] annotation for any test that accesses database
     * 2. NEVER use LspSnippetService::new() - it bypasses test environment
     * 3. ALWAYS use proper test service construction:
     *    - init_test_env() + EnvGuard::new() + setup_test_db()
     *    - Manual BookmarkServiceImpl construction with test repository
     *    - LspSnippetService::with_service() constructor
     * 
     * These tests were failing in make test-all due to factory method calls
     * trying to access production database configuration instead of test setup.
     * 
     * See CLAUDE.md and completion_service.rs tests for full documentation.
     */

    #[tokio::test]
    #[serial]
    async fn given_context_when_getting_completions_then_returns_items() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = crate::util::testing::setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(crate::application::services::bookmark_service_impl::BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(crate::infrastructure::repositories::json_import_repository::JsonImportRepository::new()),
        ));
        let snippet_service = Arc::new(LspSnippetService::with_service(bookmark_service));
        let config = BkmrConfig::default();
        let service = CompletionService::with_config(snippet_service, config);

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
        assert!(result.is_ok(), "Completion service should return Ok result");
        let items = result.expect("valid completion items");
        
        // Note: The actual number depends on database content
        // This test validates that the service doesn't crash
        println!("Got {} completion items", items.len());
    }

    #[tokio::test]
    async fn given_universal_snippet_when_translating_to_python_then_converts_comments() {
        use crate::lsp::services::LanguageTranslator;
        
        // Arrange
        let snippet = Snippet::new(
            1,
            "Universal Comment".to_string(),
            "// This is a comment\n    // Indented comment".to_string(),
            "Universal snippet".to_string(),
            vec!["universal".to_string(), "_snip_".to_string()],
        );
        let uri = Url::parse("file:///test.py").expect("parse URI");

        // Act
        let result = LanguageTranslator::translate_snippet(&snippet, "python", &uri);

        // Assert
        assert!(result.is_ok(), "Translation should succeed");
        let translated = result.expect("valid translation result");
        
        assert!(translated.contains("# This is a comment"));
        assert!(translated.contains("    # Indented comment"));
    }

    #[tokio::test]
    async fn given_plain_snippet_when_creating_completion_then_uses_plain_text_format() {
        use crate::lsp::services::CompletionService;
        use tower_lsp::lsp_types::{CompletionItemKind, InsertTextFormat};

        // Arrange
        let plain_snippet = Snippet::new(
            1,
            "Plain Text Example".to_string(),
            "simple text content with no ${1:placeholders}".to_string(),
            "Plain text snippet".to_string(),
            vec!["plain".to_string(), "_snip_".to_string()],
        );

        let repository = crate::util::testing::setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(crate::application::services::bookmark_service_impl::BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(crate::infrastructure::repositories::json_import_repository::JsonImportRepository::new()),
        ));
        let snippet_service = Arc::new(LspSnippetService::with_service(bookmark_service));
        let service = CompletionService::new(snippet_service);
        let uri = Url::parse("file:///test.rs").expect("parse URI");

        // Act
        let result = service.snippet_to_completion_item(&plain_snippet, "", None, "rust", &uri);

        // Assert
        assert!(result.is_ok(), "Completion item creation should succeed");
        let item = result.expect("valid completion item");

        assert_eq!(item.kind, Some(CompletionItemKind::TEXT));
        assert_eq!(item.insert_text_format, Some(InsertTextFormat::PLAIN_TEXT));
        assert_eq!(item.detail, Some("bkmr plain text".to_string()));
        assert_eq!(item.label, "Plain Text Example");
    }

    #[tokio::test]
    async fn given_regular_snippet_when_creating_completion_then_uses_snippet_format() {
        use crate::lsp::services::CompletionService;
        use tower_lsp::lsp_types::{CompletionItemKind, InsertTextFormat};

        // Arrange
        let regular_snippet = Snippet::new(
            1,
            "Code Snippet".to_string(),
            "snippet with ${1:placeholder}".to_string(),
            "Regular snippet with placeholders".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        let repository = crate::util::testing::setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(crate::application::services::bookmark_service_impl::BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(crate::infrastructure::repositories::json_import_repository::JsonImportRepository::new()),
        ));
        let snippet_service = Arc::new(LspSnippetService::with_service(bookmark_service));
        let service = CompletionService::new(snippet_service);
        let uri = Url::parse("file:///test.rs").expect("parse URI");

        // Act
        let result = service.snippet_to_completion_item(&regular_snippet, "", None, "rust", &uri);

        // Assert
        assert!(result.is_ok(), "Completion item creation should succeed");
        let item = result.expect("valid completion item");

        assert_eq!(item.kind, Some(CompletionItemKind::SNIPPET));
        assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
        assert_eq!(item.detail, Some("bkmr snippet".to_string()));
        assert_eq!(item.label, "Code Snippet");
    }

    #[tokio::test]
    async fn given_go_language_when_translating_rust_indentation_then_converts_to_tabs() {
        use crate::lsp::services::LanguageTranslator;
        
        // Arrange
        let uri = Url::parse("file:///test.go").expect("parse URI");
        let rust_content = "fn example() {\n    let x = 5;\n        let y = 10;\n}";

        // Act
        let result = LanguageTranslator::translate_rust_patterns(rust_content, "go", &uri);

        // Assert
        assert!(result.is_ok(), "Go translation should succeed");
        let go_result = result.expect("Go translation result");
        
        assert!(go_result.contains("fn example() {"));
        assert!(go_result.contains("\tlet x = 5;"));
        assert!(go_result.contains("\t\tlet y = 10;"));
    }

    #[tokio::test]
    async fn given_filename_template_when_translating_then_replaces_correctly() {
        use crate::lsp::services::LanguageTranslator;
        
        // Arrange
        let uri = Url::parse("file:///path/to/example.rs").expect("parse URI");
        let content = "// File: {{ filename }}";

        // Act
        let result = LanguageTranslator::translate_rust_patterns(content, "rust", &uri);

        // Assert
        assert!(result.is_ok(), "Filename template replacement should succeed");
        let translated = result.expect("valid translation result");
        
        assert!(translated.contains("// File: example.rs"));
    }

    #[tokio::test]
    #[serial]
    async fn given_health_check_when_called_then_returns_ok() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let repository = crate::util::testing::setup_test_db();
        let repository_arc = Arc::new(repository);
        let embedder = Arc::new(crate::infrastructure::embeddings::DummyEmbedding);
        let bookmark_service = Arc::new(crate::application::services::bookmark_service_impl::BookmarkServiceImpl::new(
            repository_arc,
            embedder,
            Arc::new(crate::infrastructure::repositories::json_import_repository::JsonImportRepository::new()),
        ));
        let snippet_service = Arc::new(LspSnippetService::with_service(bookmark_service));
        let service = CompletionService::new(snippet_service);

        // Act
        let result = service.health_check().await;

        // Assert
        assert!(result.is_ok(), "Health check should pass");
    }

}