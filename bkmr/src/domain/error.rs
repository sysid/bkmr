use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
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
}

pub type DomainResult<T> = Result<T, DomainError>;