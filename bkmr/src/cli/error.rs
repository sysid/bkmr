// src/cli/error.rs
use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;
use crate::exitcode;
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

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("{0}")]
    Other(String),
}

impl CliError {
    /// Process exit code for this error; the table is in ERROR_HANDLING.md.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::InvalidInput(_) | CliError::InvalidIdFormat(_) => exitcode::USAGE,
            CliError::OperationAborted => exitcode::CANCEL,
            CliError::AlreadyExists(_) => exitcode::DUP,
            _ => exitcode::FAILURE,
        }
    }

    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            CliError::CommandFailed(msg) => {
                CliError::CommandFailed(format!("{}: {}", context.into(), msg))
            }
            CliError::InvalidInput(msg) => {
                CliError::InvalidInput(format!("{}: {}", context.into(), msg))
            }
            CliError::Configuration(msg) => {
                CliError::Configuration(format!("{}: {}", context.into(), msg))
            }
            CliError::FileNotFound(msg) => {
                CliError::FileNotFound(format!("{}: {}", context.into(), msg))
            }
            CliError::PermissionDenied(msg) => {
                CliError::PermissionDenied(format!("{}: {}", context.into(), msg))
            }
            CliError::Network(msg) => CliError::Network(format!("{}: {}", context.into(), msg)),
            CliError::Application(err) => CliError::Application(err.context(context)),
            CliError::Other(msg) => CliError::Other(format!("{}: {}", context.into(), msg)),
            CliError::AlreadyExists(msg) => {
                CliError::AlreadyExists(format!("{}: {}", context.into(), msg))
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_already_exists_error_when_adding_context_then_keeps_duplicate_exit_code() {
        let err = CliError::AlreadyExists("skills/bkmr-memory".to_string()).context("install");

        assert_eq!(err.exit_code(), exitcode::DUP);
        assert_eq!(
            err.to_string(),
            "Already exists: install: skills/bkmr-memory"
        );
    }
}
