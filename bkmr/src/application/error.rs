// bkmr/src/application/error.rs
use crate::domain::error::DomainError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApplicationError {
    /// Existing presentation with the original native failure retained once.
    #[error("{presentation}")]
    Caused {
        presentation: Box<ApplicationError>,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Bookmark not found with ID {0}")]
    BookmarkNotFound(i32),

    #[error("Bookmark already exists: Id {0}: {1}")]
    BookmarkExists(i32, String),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Duplicate name '{name}' found in {file_path}. Existing bookmark with same name already exists (ID: {existing_id}). Use --update flag to overwrite existing bookmarks with changed content")]
    DuplicateName {
        name: String,
        existing_id: i32,
        file_path: String,
    },

    #[error("{0}")]
    Other(String),
}

// Add a context method for ApplicationError
impl ApplicationError {
    /// Attach an actual failure without reconstructing it from display text.
    pub fn with_source<E>(self, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Caused {
            presentation: Box::new(self),
            source: Box::new(source),
        }
    }

    /// Inspect the existing presentation category, not the original cause.
    pub fn presentation(&self) -> &Self {
        let mut current = self;
        while let Self::Caused { presentation, .. } = current {
            current = presentation;
        }
        current
    }

    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            Self::Caused { presentation, source } => Self::Caused {
                presentation: Box::new((*presentation).context(context)),
                source,
            },
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
