// src/domain/action.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use std::fmt::Debug;

/// Defines an action that can be performed on a bookmark
pub trait BookmarkAction: Debug + Send + Sync {
    /// Executes the default action for a bookmark
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()>;

    /// Returns a description of the action for UI purposes
    fn description(&self) -> &'static str;
}
