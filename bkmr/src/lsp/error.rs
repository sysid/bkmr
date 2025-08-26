//! LSP error types and conversions
//!
//! Provides error handling infrastructure for LSP operations.

use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;
use serde_json::{json, Value};
use thiserror::Error;

/// LSP-specific error type
#[derive(Error, Debug)]
pub enum LspError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl LspError {
    /// Convert error to LSP JSON response
    pub fn to_lsp_response(&self) -> Value {
        let (code, message) = match self {
            LspError::NotFound(msg) => (-32001, msg.clone()),
            LspError::InvalidInput(msg) => (-32602, msg.clone()),
            LspError::DatabaseError(msg) => (-32002, msg.clone()),
            LspError::Domain(err) => (-32003, err.to_string()),
            LspError::Application(err) => (-32004, err.to_string()),
            LspError::Internal(msg) => (-32603, msg.clone()),
        };

        json!({
            "success": false,
            "error": {
                "code": code,
                "message": message
            }
        })
    }
}

/// Result type for LSP operations
pub type LspResult<T> = Result<T, LspError>;
