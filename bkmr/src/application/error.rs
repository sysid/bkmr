// bkmr/src/application/error.rs
use crate::domain::error::DomainError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Bookmark not found with ID {0}")]
    BookmarkNotFound(i32),

    #[error("Bookmark already exists: Id {0}: {1}")]
    BookmarkExists(i32, String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("{0}")]
    Other(String),
}

// Add a context method for ApplicationError
impl ApplicationError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            ApplicationError::Other(msg) => {
                ApplicationError::Other(format!("{}: {}", context.into(), msg))
            }
            ApplicationError::Domain(err) => ApplicationError::Domain(err.context(context)),
            ApplicationError::Validation(msg) => {
                ApplicationError::Validation(format!("{}: {}", context.into(), msg))
            }
            err => ApplicationError::Other(format!("{}: {}", context.into(), err)),
        }
    }
}

impl From<std::io::Error> for ApplicationError {
    fn from(err: std::io::Error) -> Self {
        ApplicationError::Domain(DomainError::Io(err))
    }
}

impl From<std::time::SystemTimeError> for ApplicationError {
    fn from(err: std::time::SystemTimeError) -> Self {
        ApplicationError::Other(format!("System time error: {}", err))
    }
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;
