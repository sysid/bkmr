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

    #[error("Repository operation failed: {0}")]
    OperationFailed(String),
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
                crate::domain::error::DomainError::BookmarkNotFound(id.to_string())
            }
            SqliteRepositoryError::DatabaseError(diesel_err) => match diesel_err {
                DieselError::NotFound => crate::domain::error::DomainError::BookmarkNotFound(
                    "Resource not found".to_string(),
                ),
                DieselError::DatabaseError(_, info) => {
                    crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                        "Database error: {}",
                        info.message()
                    ))
                }
                _ => crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Database error: {}",
                    diesel_err
                )),
            },
            SqliteRepositoryError::ConnectionError(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Database connection error: {}",
                    e
                ))
            }
            SqliteRepositoryError::ConnectionPoolError(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Connection pool error: {}",
                    e
                ))
            }
            SqliteRepositoryError::ConversionError(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Data conversion error: {}",
                    e
                ))
            }
            SqliteRepositoryError::InvalidParameter(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Invalid parameter: {}",
                    e
                ))
            }
            SqliteRepositoryError::IoError(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "IO error: {}",
                    e
                ))
            }
            SqliteRepositoryError::MigrationError(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(format!(
                    "Migration error: {}",
                    e
                ))
            }
            SqliteRepositoryError::OperationFailed(e) => {
                crate::domain::error::DomainError::BookmarkOperationFailed(e)
            }
        }
    }
}

// This implementation is no longer needed since we have a more specific one above
// but keeping it for completeness to avoid breaking other code that might depend on it
impl From<crate::domain::error::DomainError> for SqliteRepositoryError {
    fn from(err: crate::domain::error::DomainError) -> Self {
        SqliteRepositoryError::ConversionError(err.to_string())
    }
}
