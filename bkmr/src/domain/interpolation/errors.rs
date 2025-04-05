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
