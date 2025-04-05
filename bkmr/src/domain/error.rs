// bkmr/src/domain/error.rs
use crate::domain::bookmark::BookmarkBuilderError;
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
    RepositoryError(String),

    #[error("Failed to serialize embedding: {0}")]
    SerializationError(String),

    #[error("Failed to deserialize embedding: {0}")]
    DeserializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type DomainResult<T> = Result<T, DomainError>;

impl From<BookmarkBuilderError> for DomainError {
    fn from(e: BookmarkBuilderError) -> Self {
        DomainError::BookmarkOperationFailed(e.to_string())
    }
}
