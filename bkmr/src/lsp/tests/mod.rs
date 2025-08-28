#[cfg(test)]
mod integration_tests {
    
    use crate::lsp::domain::{CompletionContext, Snippet};
    
    use crate::util::testing::{init_test_env, EnvGuard};
    
    use tower_lsp::lsp_types::{Position, Url};

    /*
     * IMPORTANT: LSP Integration Test Database Synchronization
     *
     * These integration tests require careful database access patterns:
     *
     * 1. Tests run single-threaded (--test-threads=1) so no special synchronization needed
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
    async fn given_context_when_getting_completions_then_returns_items() {
        // Arrange
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        let service = lsp_services.completion_service;

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
        
        use tower_lsp::lsp_types::{CompletionItemKind, InsertTextFormat};

        // Arrange
        let plain_snippet = Snippet::new(
            1,
            "Plain Text Example".to_string(),
            "simple text content with no ${1:placeholders}".to_string(),
            "Plain text snippet".to_string(),
            vec!["plain".to_string(), "_snip_".to_string()],
        );

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        let service = lsp_services.completion_service;
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
        
        use tower_lsp::lsp_types::{CompletionItemKind, InsertTextFormat};

        // Arrange
        let regular_snippet = Snippet::new(
            1,
            "Code Snippet".to_string(),
            "snippet with ${1:placeholder}".to_string(),
            "Regular snippet with placeholders".to_string(),
            vec!["rust".to_string(), "_snip_".to_string()],
        );

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        let service = lsp_services.completion_service;
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
        assert!(
            result.is_ok(),
            "Filename template replacement should succeed"
        );
        let translated = result.expect("valid translation result");

        assert!(translated.contains("// File: example.rs"));
    }

    #[tokio::test]
    async fn given_health_check_when_called_then_returns_ok() {
        // Arrange
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        let service = lsp_services.completion_service;

        // Act
        let result = service.health_check().await;

        // Assert
        assert!(result.is_ok(), "Health check should pass");
    }

    #[tokio::test]
    async fn given_create_snippet_command_when_executed_then_creates_snippet() {
        // Arrange
        use crate::lsp::backend::{BkmrConfig, BkmrLspBackend};
        
        use serde_json::json;
        use tower_lsp::lsp_types::ExecuteCommandParams;
        use tower_lsp::{Client, LanguageServer};

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        // Use services from container

        let (service, _socket) = tower_lsp::LspService::new(|client: Client| {
            BkmrLspBackend::with_services(
                client,
                BkmrConfig::default(),
                lsp_services.completion_service,
                lsp_services.document_service,
                lsp_services.command_service,
            )
        });
        let backend = service.inner();

        let params = ExecuteCommandParams {
            command: "bkmr.createSnippet".to_string(),
            arguments: vec![json!({
                "url": "test snippet content",
                "title": "Test Snippet",
                "description": "A test snippet",
                "tags": ["rust", "test"]
            })],
            work_done_progress_params: Default::default(),
        };

        // Act
        let result = backend.execute_command(params).await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());
        let json = response.unwrap();
        assert!(json.get("id").is_some());
        assert_eq!(json.get("title").unwrap().as_str().unwrap(), "Test Snippet");
    }

    #[tokio::test]
    async fn given_list_snippets_command_when_executed_then_returns_filtered_list() {
        // Arrange
        use crate::lsp::backend::{BkmrConfig, BkmrLspBackend};
        
        use serde_json::json;
        use tower_lsp::lsp_types::ExecuteCommandParams;
        use tower_lsp::{Client, LanguageServer};

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        // Use services from container

        let (service, _socket) = tower_lsp::LspService::new(|client: Client| {
            BkmrLspBackend::with_services(
                client,
                BkmrConfig::default(),
                lsp_services.completion_service,
                lsp_services.document_service,
                lsp_services.command_service,
            )
        });
        let backend = service.inner();

        // First create some snippets
        backend
            .execute_command(ExecuteCommandParams {
                command: "bkmr.createSnippet".to_string(),
                arguments: vec![json!({
                    "url": "fn rust_fn() {}",
                    "title": "Rust Function",
                    "tags": ["rust"]
                })],
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap();

        backend
            .execute_command(ExecuteCommandParams {
                command: "bkmr.createSnippet".to_string(),
                arguments: vec![json!({
                    "url": "def python_fn():",
                    "title": "Python Function",
                    "tags": ["python"]
                })],
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap();

        // Act - List only Rust snippets
        let params = ExecuteCommandParams {
            command: "bkmr.listSnippets".to_string(),
            arguments: vec![json!({
                "language": "rust"
            })],
            work_done_progress_params: Default::default(),
        };

        let result = backend.execute_command(params).await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());
        let json = response.unwrap();
        let snippets = json.get("snippets").unwrap().as_array().unwrap();

        // Should only contain Rust snippets
        for snippet in snippets {
            let tags = snippet.get("tags").unwrap().as_array().unwrap();
            assert!(tags.iter().any(|t| t.as_str() == Some("rust")));
        }
    }

    #[tokio::test]
    async fn given_update_snippet_command_when_executed_then_updates_and_preserves_system_tag() {
        // Arrange
        use crate::lsp::backend::{BkmrConfig, BkmrLspBackend};
        
        use serde_json::json;
        use tower_lsp::lsp_types::ExecuteCommandParams;
        use tower_lsp::{Client, LanguageServer};

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        // Use services from container

        let (service, _socket) = tower_lsp::LspService::new(|client: Client| {
            BkmrLspBackend::with_services(
                client,
                BkmrConfig::default(),
                lsp_services.completion_service,
                lsp_services.document_service,
                lsp_services.command_service,
            )
        });
        let backend = service.inner();

        // Create a snippet first
        let create_result = backend
            .execute_command(ExecuteCommandParams {
                command: "bkmr.createSnippet".to_string(),
                arguments: vec![json!({
                    "url": "original content",
                    "title": "Original Title",
                    "tags": ["rust"]
                })],
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap()
            .unwrap();

        let id = create_result.get("id").unwrap().as_i64().unwrap();

        // Act - Update the snippet
        let params = ExecuteCommandParams {
            command: "bkmr.updateSnippet".to_string(),
            arguments: vec![json!({
                "id": id,
                "url": "updated content",
                "title": "Updated Title",
                "tags": ["python", "updated"]
            })],
            work_done_progress_params: Default::default(),
        };

        let result = backend.execute_command(params).await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());
        let json = response.unwrap();

        assert_eq!(
            json.get("title").unwrap().as_str().unwrap(),
            "Updated Title"
        );
        assert_eq!(
            json.get("url").unwrap().as_str().unwrap(),
            "updated content"
        );

        let tags = json.get("tags").unwrap().as_array().unwrap();
        assert!(tags.iter().any(|t| t.as_str() == Some("_snip_"))); // System tag preserved
        assert!(tags.iter().any(|t| t.as_str() == Some("python")));
        assert!(tags.iter().any(|t| t.as_str() == Some("updated")));
    }

    #[tokio::test]
    async fn given_delete_snippet_command_when_executed_then_deletes_snippet() {
        // Arrange
        use crate::lsp::backend::{BkmrConfig, BkmrLspBackend};
        
        use serde_json::json;
        use tower_lsp::lsp_types::ExecuteCommandParams;
        use tower_lsp::{Client, LanguageServer};

        // Use centralized test service container
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        let lsp_services = test_container.create_lsp_services();
        // Use services from container

        let (service, _socket) = tower_lsp::LspService::new(|client: Client| {
            BkmrLspBackend::with_services(
                client,
                BkmrConfig::default(),
                lsp_services.completion_service,
                lsp_services.document_service,
                lsp_services.command_service,
            )
        });
        let backend = service.inner();

        // Create a snippet first
        let create_result = backend
            .execute_command(ExecuteCommandParams {
                command: "bkmr.createSnippet".to_string(),
                arguments: vec![json!({
                    "url": "to be deleted",
                    "title": "To Delete",
                    "tags": ["temp"]
                })],
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap()
            .unwrap();

        let id = create_result.get("id").unwrap().as_i64().unwrap();

        // Act - Delete the snippet
        let params = ExecuteCommandParams {
            command: "bkmr.deleteSnippet".to_string(),
            arguments: vec![json!({
                "id": id
            })],
            work_done_progress_params: Default::default(),
        };

        let result = backend.execute_command(params).await;

        // Assert
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());
        let json = response.unwrap();
        assert!(json.get("success").unwrap().as_bool().unwrap());

        // Verify it's deleted by trying to get it
        let get_params = ExecuteCommandParams {
            command: "bkmr.getSnippet".to_string(),
            arguments: vec![json!({
                "id": id
            })],
            work_done_progress_params: Default::default(),
        };

        let get_result = backend.execute_command(get_params).await.unwrap().unwrap();
        assert!(!get_result.get("success").unwrap().as_bool().unwrap());
    }
}
