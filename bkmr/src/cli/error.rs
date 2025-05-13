// src/cli/error.rs
use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid ID format: {0}")]
    InvalidIdFormat(String),

    #[error("Operation aborted by user")]
    OperationAborted,

    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Other(String),
}

// Add context method to CliError
impl CliError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            CliError::CommandFailed(msg) => {
                CliError::CommandFailed(format!("{}: {}", context.into(), msg))
            }
            CliError::InvalidInput(msg) => {
                CliError::InvalidInput(format!("{}: {}", context.into(), msg))
            }
            CliError::Application(err) => CliError::Application(err.context(context)),
            CliError::Other(msg) => CliError::Other(format!("{}: {}", context.into(), msg)),
            err => CliError::Other(format!("{}: {}", context.into(), err)),
        }
    }
}

// Direct conversion from DomainError to CliError (via ApplicationError)
impl From<DomainError> for CliError {
    fn from(err: DomainError) -> Self {
        CliError::Application(ApplicationError::Domain(err))
    }
}

impl From<crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError> for CliError {
    fn from(
        err: crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError,
    ) -> Self {
        // Convert via DomainError which already has a From implementation for SqliteRepositoryError
        CliError::Application(ApplicationError::Domain(err.into()))
    }
}

pub type CliResult<T> = Result<T, CliError>;
