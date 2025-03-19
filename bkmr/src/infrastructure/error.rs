use crate::domain::error::DomainError;
use thiserror::Error;

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

    #[error("Repository error: {0}")]
    Repository(String),
}

// Implement conversion from infrastructure errors to domain errors
impl From<InfrastructureError> for DomainError {
    fn from(error: InfrastructureError) -> Self {
        match error {
            InfrastructureError::Database(msg) => DomainError::BookmarkOperationFailed(msg),
            InfrastructureError::Network(msg) => DomainError::CannotFetchMetadata(msg),
            InfrastructureError::Serialization(msg) => DomainError::BookmarkOperationFailed(msg),
            InfrastructureError::FileSystem(msg) => DomainError::BookmarkOperationFailed(msg),
            InfrastructureError::Repository(msg) => DomainError::RepositoryError(msg),
        }
    }
}

// Add conversions from specific repository errors
impl From<crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError>
    for InfrastructureError
{
    fn from(
        error: crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError,
    ) -> Self {
        InfrastructureError::Database(error.to_string())
    }
}
