use thiserror::Error;
use crate::domain::error::DomainError;

#[derive(Error, Debug)]
pub enum InfrastructureError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("File system error: {0}")]
    FileSystem(String),
}

// Then implement From to convert to domain errors
impl From<InfrastructureError> for DomainError {
    fn from(error: InfrastructureError) -> Self {
        match error {
            InfrastructureError::Database(msg) => DomainError::BookmarkOperationFailed(msg),
            InfrastructureError::Network(msg) => DomainError::CannotFetchMetadata(msg),
            InfrastructureError::Serialization(msg) => DomainError::BookmarkOperationFailed(msg),
            InfrastructureError::FileSystem(msg) => DomainError::BookmarkOperationFailed(msg),
        }
    }
}
