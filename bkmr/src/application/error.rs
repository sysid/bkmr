use thiserror::Error;
use crate::domain::error::DomainError;

/// High-level errors for the application layer.
#[derive(Error, Debug)]
pub enum ApplicationError {
    /// A bookmark with a particular ID was not found.
    #[error("Bookmark not found with ID {0}")]
    BookmarkNotFound(i32),

    /// A bookmark with this URL already exists.
    #[error("Bookmark with this URL already exists: {0}")]
    BookmarkExists(String),

    /// Wraps domain-level errors (e.g., validation, parsing).
    #[error("Domain error occurred: {0}")]
    Domain(#[from] DomainError),

    /// Validation or business-logic errors specific to application logic.
    #[error("Validation failed: {0}")]
    Validation(String),

    /// Catch-all for anything else in the application layer.
    #[error("Other application error: {0}")]
    Other(String),
}

/// Result alias for application services.
pub type ApplicationResult<T> = std::result::Result<T, ApplicationError>;
