// src/domain/interpolation/errors.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InterpolationError {
    #[error("Template syntax error: {0}")]
    Syntax(String),

    #[error("Template rendering error: {0}")]
    Rendering(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Shell command error: {0}")]
    Shell(String),
}

// Add context method
impl InterpolationError {
    pub fn context<C: Into<String>>(self, context: C) -> Self {
        match self {
            InterpolationError::Syntax(msg) => {
                InterpolationError::Syntax(format!("{}: {}", context.into(), msg))
            }
            InterpolationError::Rendering(msg) => {
                InterpolationError::Rendering(format!("{}: {}", context.into(), msg))
            }
            InterpolationError::Context(msg) => {
                InterpolationError::Context(format!("{}: {}", context.into(), msg))
            }
            InterpolationError::Shell(msg) => {
                InterpolationError::Shell(format!("{}: {}", context.into(), msg))
            }
        }
    }
}
