// src/cli/error.rs

use std::io;
use thiserror::Error;

use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;
use crate::infrastructure::repositories::sqlite::error::SqliteRepositoryError;

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

    #[error("Repository error: {0}")]
    RepositoryError(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] SqliteRepositoryError),

    #[error("Other error: {0}")]
    Other(String),
}

pub type CliResult<T> = Result<T, CliError>;

// impl From<DomainError> for CliError {
//     fn from(err: DomainError) -> Self {
//         CliError::Domain(err)
//     }
// }
//
// impl From<ApplicationError> for CliError {
//     fn from(err: ApplicationError) -> Self {
//         CliError::Application(err)
//     }
// }
//
// impl From<SqliteRepositoryError> for CliError {
//     fn from(err: SqliteRepositoryError) -> Self {
//         CliError::Sqlite(err)
//     }
// }
