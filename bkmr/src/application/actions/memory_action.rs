// src/application/actions/memory_action.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::DomainResult;
use tracing::instrument;

/// Action for agent memory bookmarks: display content to stdout
#[derive(Debug)]
pub struct MemoryAction;

impl MemoryAction {
    pub fn new() -> Self {
        Self
    }
}

impl BookmarkAction for MemoryAction {
    #[instrument(skip(self, bookmark), level = "debug")]
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        println!("{}", bookmark.url);
        Ok(())
    }

    fn description(&self) -> &'static str {
        "Display memory content"
    }
}
