use crate::application::error::ApplicationError;
use crate::cli::error::CliError;
use crate::domain::error::DomainError;

/// Trait for providing contextual error information
///
/// This trait standardizes error context handling across the application
/// by providing a consistent way to add context to errors without
/// manual string formatting.
pub trait ErrorContext<T> {
    /// Add context to an error result
    fn with_context<F>(self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce() -> String;

    /// Add context to an error result with a static string
    fn context(self, msg: &'static str) -> Result<T, DomainError>;
}

/// Application-level error context
pub trait ApplicationErrorContext<T> {
    /// Add context to an application error result
    fn with_app_context<F>(self, f: F) -> Result<T, ApplicationError>
    where
        F: FnOnce() -> String;

    /// Add context to an application error result with a static string
    fn app_context(self, msg: &'static str) -> Result<T, ApplicationError>;
}

/// CLI-level error context
pub trait CliErrorContext<T> {
    /// Add context to a CLI error result
    fn with_cli_context<F>(self, f: F) -> Result<T, CliError>
    where
        F: FnOnce() -> String;

    /// Add context to a CLI error result with a static string
    fn cli_context(self, msg: &'static str) -> Result<T, CliError>;
}

// Implementation for DomainError results
impl<T, E> ErrorContext<T> for Result<T, E>
where
    E: Into<DomainError>,
{
    fn with_context<F>(self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into().context(f()))
    }

    fn context(self, msg: &'static str) -> Result<T, DomainError> {
        self.map_err(|e| e.into().context(msg))
    }
}

// Implementation for ApplicationError results
impl<T, E> ApplicationErrorContext<T> for Result<T, E>
where
    E: Into<ApplicationError>,
{
    fn with_app_context<F>(self, f: F) -> Result<T, ApplicationError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into().context(f()))
    }

    fn app_context(self, msg: &'static str) -> Result<T, ApplicationError> {
        self.map_err(|e| e.into().context(msg))
    }
}

// Implementation for CliError results
impl<T, E> CliErrorContext<T> for Result<T, E>
where
    E: Into<CliError>,
{
    fn with_cli_context<F>(self, f: F) -> Result<T, CliError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.into().context(f()))
    }

    fn cli_context(self, msg: &'static str) -> Result<T, CliError> {
        self.map_err(|e| e.into().context(msg))
    }
}

/// Convenience functions for common error conversions
pub trait ErrorConversion {
    /// Convert to DomainError with context
    fn to_domain_error(self, context: &'static str) -> DomainError;

    /// Convert to ApplicationError with context
    fn to_app_error(self, context: &'static str) -> ApplicationError;

    /// Convert to CliError with context
    fn to_cli_error(self, context: &'static str) -> CliError;
}

// Implement for common error types
impl ErrorConversion for std::io::Error {
    fn to_domain_error(self, context: &'static str) -> DomainError {
        DomainError::Io(self).context(context)
    }

    fn to_app_error(self, context: &'static str) -> ApplicationError {
        ApplicationError::Domain(DomainError::Io(self)).context(context)
    }

    fn to_cli_error(self, context: &'static str) -> CliError {
        CliError::Io(self).context(context)
    }
}

impl ErrorConversion for serde_json::Error {
    fn to_domain_error(self, context: &'static str) -> DomainError {
        DomainError::DeserializationError(self.to_string()).context(context)
    }

    fn to_app_error(self, context: &'static str) -> ApplicationError {
        ApplicationError::Domain(DomainError::DeserializationError(self.to_string()))
            .context(context)
    }

    fn to_cli_error(self, context: &'static str) -> CliError {
        CliError::Application(ApplicationError::Domain(DomainError::DeserializationError(
            self.to_string(),
        )))
        .context(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn given_io_error_when_add_context_then_returns_formatted_error() {
        let result: Result<(), io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        let contextual_result = result.context("reading configuration");

        assert!(contextual_result.is_err());
        assert!(contextual_result
            .unwrap_err()
            .to_string()
            .contains("reading configuration"));
    }

    #[test]
    fn given_application_error_when_add_context_then_returns_contextual_error() {
        let result: Result<(), ApplicationError> =
            Err(ApplicationError::Other("test error".to_string()));
        let contextual_result = result.app_context("during operation");

        assert!(contextual_result.is_err());
        assert!(contextual_result
            .unwrap_err()
            .to_string()
            .contains("during operation"));
    }

    #[test]
    fn given_error_when_convert_with_context_then_preserves_message() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let domain_error = io_error.to_domain_error("file operation");

        assert!(domain_error.to_string().contains("file operation"));
        assert!(domain_error.to_string().contains("access denied"));
    }
}
