// src/domain/action.rs
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use std::fmt::Debug;

/// How one run of an action goes. Only shell scripts use these today.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExecOptions<'a> {
    /// Run without the interactive edit step (`bkmr open --no-edit`)
    pub no_edit: bool,
    /// Arguments for a shell script (`bkmr open ID -- ARGS`)
    pub script_args: &'a [String],
}

/// Defines an action that can be performed on a bookmark
pub trait BookmarkAction: Debug + Send + Sync {
    /// Executes the default action for a bookmark
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()>;

    /// Executes the default action with run options. Actions that have no use for them keep
    /// this default.
    fn execute_with(&self, bookmark: &Bookmark, _options: ExecOptions) -> DomainResult<()> {
        self.execute(bookmark)
    }

    /// Returns a description of the action for UI purposes
    fn description(&self) -> &'static str;
}
