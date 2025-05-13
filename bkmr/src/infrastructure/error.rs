// src/infrastructure/error.rs
use crate::domain::error::{DomainError, RepositoryError};
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
    Repository(#[from] RepositoryError),
}

// Add context method
impl InfrastructureError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            InfrastructureError::Database(msg) => {
                InfrastructureError::Database(format!("{}: {}", context.into(), msg))
            }
            InfrastructureError::Network(msg) => {
                InfrastructureError::Network(format!("{}: {}", context.into(), msg))
            }
            InfrastructureError::Repository(err) => {
                InfrastructureError::Repository(err.context(context))
            }
            err => InfrastructureError::Database(format!("{}: {}", context.into(), err)),
        }
    }
}

// Convert to domain errors
impl From<InfrastructureError> for DomainError {
    fn from(error: InfrastructureError) -> Self {
        match error {
            InfrastructureError::Database(msg) => {
                DomainError::RepositoryError(RepositoryError::Database(msg))
            }
            InfrastructureError::Network(msg) => DomainError::CannotFetchMetadata(msg),
            InfrastructureError::Serialization(msg) => DomainError::SerializationError(msg),
            InfrastructureError::FileSystem(msg) => {
                DomainError::Io(std::io::Error::new(std::io::ErrorKind::Other, msg))
            }
            InfrastructureError::Repository(err) => DomainError::RepositoryError(err),
        }
    }
}
