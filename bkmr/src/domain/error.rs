// src/domain/error.rs
use crate::domain::bookmark::BookmarkBuilderError;
use crate::domain::interpolation;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Invalid tag: {0}")]
    InvalidTag(String),

    #[error("Tag operation failed: {0}")]
    TagOperationFailed(String),

    #[error("Bookmark operation failed: {0}")]
    BookmarkOperationFailed(String),

    #[error("Bookmark not found: {0}")]
    BookmarkNotFound(String),

    #[error("Cannot fetch metadata: {0}")]
    CannotFetchMetadata(String),

    #[error("Repository error: {0}")]
    RepositoryError(#[from] RepositoryError),

    #[error("Interpolation error: {0}")]
    Interpolation(#[from] interpolation::errors::InterpolationError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("{0}")]
    Other(String),
}

// New repository error enum to represent generic repository errors
#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),

    #[error("Repository error: {0}")]
    Other(String),
}

// Add a context method to DomainError for better error context
impl DomainError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            DomainError::Other(msg) => DomainError::Other(format!("{}: {}", context.into(), msg)),
            DomainError::BookmarkOperationFailed(msg) => {
                DomainError::BookmarkOperationFailed(format!("{}: {}", context.into(), msg))
            }
            DomainError::RepositoryError(err) => {
                DomainError::RepositoryError(RepositoryError::context(err, context))
            }
            // Add more specific handling for other error types as needed
            err => DomainError::Other(format!("{}: {}", context.into(), err)),
        }
    }
}

// Add a context method to RepositoryError
impl RepositoryError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            RepositoryError::Other(msg) => {
                RepositoryError::Other(format!("{}: {}", context.into(), msg))
            }
            RepositoryError::Database(msg) => {
                RepositoryError::Database(format!("{}: {}", context.into(), msg))
            }
            RepositoryError::NotFound(msg) => {
                RepositoryError::NotFound(format!("{}: {}", context.into(), msg))
            }
            err => RepositoryError::Other(format!("{}: {}", context.into(), err)),
        }
    }
}

// Common result type
pub type DomainResult<T> = Result<T, DomainError>;

impl From<BookmarkBuilderError> for DomainError {
    fn from(e: BookmarkBuilderError) -> Self {
        DomainError::BookmarkOperationFailed(e.to_string())
    }
}

impl From<crate::lsp::services::SnippetError> for DomainError {
    fn from(e: crate::lsp::services::SnippetError) -> Self {
        DomainError::Other(e.to_string())
    }
}
