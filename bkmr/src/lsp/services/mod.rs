//! LSP service layer adapters
//! 
//! This module provides async wrappers around the existing bkmr services
//! to enable their use in the LSP server context.

#[cfg(feature = "lsp")]
pub mod completion_service;
#[cfg(feature = "lsp")]
pub mod language_translator;
#[cfg(feature = "lsp")]
pub mod snippet_service;

#[cfg(feature = "lsp")]
pub use completion_service::*;
#[cfg(feature = "lsp")]
pub use language_translator::*;
#[cfg(feature = "lsp")]
pub use snippet_service::*;