//! LSP (Language Server Protocol) implementation for bkmr
//! 
//! This module provides LSP server functionality for snippet completion
//! and editing in editors that support the Language Server Protocol.

#[cfg(feature = "lsp")]
pub mod backend;
#[cfg(feature = "lsp")]
pub mod domain;
#[cfg(feature = "lsp")]
pub mod services;

#[cfg(feature = "lsp")]
pub use backend::BkmrLspBackend;

#[cfg(feature = "lsp")]
pub async fn run_lsp_server(no_interpolation: bool) {
    backend::run_server(no_interpolation).await;
}

#[cfg(not(feature = "lsp"))]
pub async fn run_lsp_server(_no_interpolation: bool) {
    eprintln!("LSP support not compiled. Build with --features lsp");
    std::process::exit(1);
}