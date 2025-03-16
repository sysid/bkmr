// src/cli/error.rs
use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Operation aborted by user")]
    OperationAborted,

    #[error("Invalid ID format: {0}")]
    InvalidIdFormat(String),

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type CliResult<T> = Result<T, CliError>;

// Utility function to convert string errors to CliError
pub fn into_cli_error<E: std::error::Error>(err: E, context: &str) -> CliError {
    CliError::CommandFailed(format!("{}: {}", context, err))
}
