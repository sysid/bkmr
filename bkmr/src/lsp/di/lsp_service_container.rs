use crate::application::services::bookmark_service::BookmarkService;
use crate::application::services::InterpolationService;
use crate::lsp::backend::BkmrConfig;
use crate::lsp::services::{CommandService, CompletionService, DocumentService, LspSnippetService};
use std::sync::Arc;

/// LSP-specific service container for editor integration
pub struct LspServiceContainer {
    pub completion_service: CompletionService,
    pub command_service: CommandService,
    pub document_service: DocumentService,
}

impl LspServiceContainer {
    pub fn new(
        bookmark_service: Arc<dyn BookmarkService>,
        interpolation_service: Arc<dyn InterpolationService>,
        config: BkmrConfig,
    ) -> Self {
        let snippet_service = Arc::new(LspSnippetService::with_services(
            bookmark_service.clone(),
            interpolation_service,
        ));

        Self {
            completion_service: CompletionService::with_config(snippet_service, config),
            command_service: CommandService::with_service(bookmark_service),
            document_service: DocumentService::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::domain::CompletionContext;
    use crate::util::testing::{init_test_env, EnvGuard};
    use tower_lsp_server::ls_types::{Position, Uri};

    fn container_with_interpolation(enabled: bool) -> LspServiceContainer {
        let test_container = crate::util::test_service_container::TestServiceContainer::new();
        LspServiceContainer::new(
            test_container.bookmark_service.clone(),
            test_container.interpolation_service.clone(),
            BkmrConfig {
                max_completions: 50,
                enable_interpolation: enabled,
            },
        )
    }

    async fn completion_texts(container: &LspServiceContainer, title: &str) -> String {
        let uri = "file:///test.txt".parse::<Uri>().expect("parse URI");
        let context = CompletionContext::new(
            uri,
            Position {
                line: 0,
                character: 0,
            },
            None,
        );
        let items = container
            .completion_service
            .get_completions(&context)
            .await
            .expect("completions");
        let item = items
            .iter()
            .find(|i| i.label == title)
            .unwrap_or_else(|| panic!("snippet '{}' not found in completions", title));
        item.insert_text.clone().expect("insert text")
    }

    #[tokio::test]
    async fn given_interpolation_disabled_when_completing_then_returns_raw_template() {
        // Regression: `bkmr lsp --no-interpolation` was silently ignored because
        // run_server never passed its BkmrConfig to the CompletionService.
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let container = container_with_interpolation(false);
        container
            .command_service
            .create_snippet(
                "value: {{ \"rendered\" }}",
                "Interp Wiring Test",
                None,
                vec![],
            )
            .expect("create snippet");

        let text = completion_texts(&container, "Interp Wiring Test").await;

        assert!(
            text.contains("{{"),
            "content must stay raw when interpolation is disabled, got: {}",
            text
        );
    }

    #[tokio::test]
    async fn given_interpolation_enabled_when_completing_then_returns_rendered_template() {
        let _env = init_test_env();
        let _guard = EnvGuard::new();
        let container = container_with_interpolation(true);
        container
            .command_service
            .create_snippet(
                "value: {{ \"rendered\" }}",
                "Interp Wiring Test",
                None,
                vec![],
            )
            .expect("create snippet");

        let text = completion_texts(&container, "Interp Wiring Test").await;

        assert!(
            text.contains("value: rendered"),
            "template must render when interpolation is enabled, got: {}",
            text
        );
    }
}
