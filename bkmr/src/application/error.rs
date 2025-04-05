// bkmr/src/application/error.rs
use crate::domain::error::DomainError;
use thiserror::Error;

/// High-level errors for the application layer.
#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Bookmark not found with ID {0}")]
    BookmarkNotFound(i32),

    #[error("Bookmark with this URL already exists: {0}")]
    BookmarkExists(String),

    #[error("Domain error occurred: {0}")]
    Domain(#[from] DomainError),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Other application error: {0}")]
    Other(String),
}

/// Result alias for application services.
pub type ApplicationResult<T> = Result<T, ApplicationError>;

impl From<std::io::Error> for ApplicationError {
    fn from(err: std::io::Error) -> Self {
        ApplicationError::Other(format!("IO error: {}", err))
    }
}

impl From<std::time::SystemTimeError> for ApplicationError {
    fn from(err: std::time::SystemTimeError) -> Self {
        ApplicationError::Other(format!("System time error: {}", err))
    }
}
