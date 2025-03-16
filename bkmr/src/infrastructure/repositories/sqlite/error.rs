// src/infrastructure/repositories/sqlite/error.rs

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
}

pub type SqliteResult<T> = Result<T, SqliteRepositoryError>;

impl From<r2d2::Error> for SqliteRepositoryError {
    fn from(err: r2d2::Error) -> Self {
        SqliteRepositoryError::ConnectionPoolError(err.to_string())
    }
}

impl From<SqliteRepositoryError> for crate::domain::error::DomainError {
    fn from(err: SqliteRepositoryError) -> Self {
        match err {
            SqliteRepositoryError::BookmarkNotFound(id) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Bookmark not found with ID: {}",
                    id
                ))
            }
            _ => crate::domain::error::DomainError::BookmarkOperationFailed(err.to_string()),
        }
    }
}

impl From<crate::domain::error::DomainError> for SqliteRepositoryError {
    fn from(err: crate::domain::error::DomainError) -> Self {
        SqliteRepositoryError::ConversionError(err.to_string())
    }
}
