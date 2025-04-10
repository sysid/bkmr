// src/domain/interpolation/interface.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::interpolation::errors::InterpolationError;
use std::sync::Arc;

pub trait InterpolationEngine: Send + Sync + std::fmt::Debug {
    fn render_url(&self, url: &str) -> Result<String, InterpolationError>;
    fn render_bookmark_url(&self, bookmark: &Bookmark) -> Result<String, InterpolationError>;
}

pub trait ShellCommandExecutor: Send + Sync + std::fmt::Debug {
    fn execute(&self, command: &str) -> Result<String, InterpolationError>;
    fn arc_clone(&self) -> Arc<dyn ShellCommandExecutor>;
}
