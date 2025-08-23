//! LSP (Language Server Protocol) implementation for bkmr
//! 
//! This module provides LSP server functionality for snippet completion
//! and editing in editors that support the Language Server Protocol.

pub mod backend;
pub mod domain;
pub mod error;
pub mod services;

#[cfg(test)]
mod tests;

pub use backend::BkmrLspBackend;

pub async fn run_lsp_server(no_interpolation: bool) {
    backend::run_server(no_interpolation).await;
}