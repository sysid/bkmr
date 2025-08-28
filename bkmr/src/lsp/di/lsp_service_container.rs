use crate::infrastructure::di::ServiceContainer;
use crate::lsp::services::{CommandService, CompletionService, DocumentService, LspSnippetService};
use crate::config::Settings;
use std::sync::Arc;

/// LSP-specific service container for editor integration
pub struct LspServiceContainer {
    pub completion_service: CompletionService,
    pub command_service: CommandService,
    pub document_service: DocumentService,
}

impl LspServiceContainer {
    pub fn new(service_container: &ServiceContainer, _config: &Settings) -> Self {
        // Create LSP-specific services with explicit dependencies
        let snippet_service = Arc::new(LspSnippetService::with_services(
            service_container.bookmark_service.clone(),
            service_container.template_service.clone(),
        ));
        
        Self {
            completion_service: CompletionService::new(snippet_service.clone()),
            command_service: CommandService::with_service(
                service_container.bookmark_service.clone()
            ),
            document_service: DocumentService::new(),
        }
    }
}