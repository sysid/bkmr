// src/infrastructure/repositories/sqlite/error.rs
use crate::domain::error::{DomainError, RepositoryError};
use diesel::r2d2;
use diesel::result::Error as DieselError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqliteRepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] DieselError),

    #[error("Diesel connection error: {0}")]
    ConnectionError(#[from] diesel::ConnectionError),

    #[error("Connection pool error: {0}")]
    ConnectionPoolError(String),

    #[error("Bookmark not found with ID: {0}")]
    BookmarkNotFound(i32),

    #[error("Failed to convert entity: {0}")]
    ConversionError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("{0}")]
    OperationFailed(String),
}

pub type SqliteResult<T> = Result<T, SqliteRepositoryError>;

// Add a context method to SqliteRepositoryError
impl SqliteRepositoryError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            SqliteRepositoryError::OperationFailed(msg) => {
                SqliteRepositoryError::OperationFailed(format!("{}: {}", context.into(), msg))
            }
            SqliteRepositoryError::ConversionError(msg) => {
                SqliteRepositoryError::ConversionError(format!("{}: {}", context.into(), msg))
            }
            err => SqliteRepositoryError::OperationFailed(format!("{}: {}", context.into(), err)),
        }
    }
}

impl From<r2d2::Error> for SqliteRepositoryError {
    fn from(err: r2d2::Error) -> Self {
        SqliteRepositoryError::ConnectionPoolError(err.to_string())
    }
}

// Convert SQLite errors to domain RepositoryError
impl From<SqliteRepositoryError> for RepositoryError {
    fn from(err: SqliteRepositoryError) -> Self {
        match err {
            SqliteRepositoryError::BookmarkNotFound(id) => {
                RepositoryError::NotFound(format!("Bookmark with ID {}", id))
            }
            SqliteRepositoryError::DatabaseError(diesel_err) => match diesel_err {
                DieselError::NotFound => {
                    RepositoryError::NotFound("Resource not found".to_string())
                }
                DieselError::DatabaseError(_, info) => {
                    RepositoryError::Database(format!("Database error: {}", info.message()))
                }
                _ => RepositoryError::Database(format!("Database error: {}", diesel_err)),
            },
            SqliteRepositoryError::ConnectionError(e) => {
                RepositoryError::Connection(format!("Database connection error: {}", e))
            }
            SqliteRepositoryError::ConnectionPoolError(e) => {
                RepositoryError::Connection(format!("Connection pool error: {}", e))
            }
            SqliteRepositoryError::ConversionError(e) => {
                RepositoryError::Other(format!("Data conversion error: {}", e))
            }
            SqliteRepositoryError::InvalidParameter(e) => {
                RepositoryError::Other(format!("Invalid parameter: {}", e))
            }
            SqliteRepositoryError::IoError(e) => RepositoryError::Other(format!("IO error: {}", e)),
            SqliteRepositoryError::MigrationError(e) => {
                RepositoryError::Other(format!("Migration error: {}", e))
            }
            SqliteRepositoryError::OperationFailed(e) => RepositoryError::Other(e),
        }
    }
}

// Simplified conversion from SqliteRepositoryError to DomainError
impl From<SqliteRepositoryError> for DomainError {
    fn from(err: SqliteRepositoryError) -> Self {
        DomainError::RepositoryError(err.into())
    }
}
