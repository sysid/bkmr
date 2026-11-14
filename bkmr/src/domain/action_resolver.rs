// src/domain/action_resolver.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::system_tag::SystemTag;
use std::fmt::Debug;

/// Resolves the appropriate action for a bookmark based on its tags or other properties
pub trait ActionResolver: Debug + Send + Sync {
    /// Resolves the default action for a bookmark
    fn resolve_action(&self, bookmark: &Bookmark) -> &dyn BookmarkAction;
}

/// Default implementation that uses the SystemTag to determine the action
#[derive(Debug)]
pub struct SystemTagActionResolver {
    uri_action: Box<dyn BookmarkAction>,
    snippet_action: Box<dyn BookmarkAction>,
    text_action: Box<dyn BookmarkAction>,
    shell_action: Box<dyn BookmarkAction>,
    markdown_action: Box<dyn BookmarkAction>,
    env_action: Box<dyn BookmarkAction>,
    memory_action: Box<dyn BookmarkAction>,
}

impl SystemTagActionResolver {
    pub fn new(
        uri_action: Box<dyn BookmarkAction>,
        snippet_action: Box<dyn BookmarkAction>,
        text_action: Box<dyn BookmarkAction>,
        shell_action: Box<dyn BookmarkAction>,
        markdown_action: Box<dyn BookmarkAction>,
        env_action: Box<dyn BookmarkAction>,
        memory_action: Box<dyn BookmarkAction>,
    ) -> Self {
        Self {
            uri_action,
            snippet_action,
            text_action,
            shell_action,
            markdown_action,
            env_action,
            memory_action,
        }
    }
}

impl ActionResolver for SystemTagActionResolver {
    fn resolve_action(&self, bookmark: &Bookmark) -> &dyn BookmarkAction {
        // A bookmark without a known content-type tag is a URI (Bookmark::is_uri)
        let action = if bookmark.is_snippet() {
            &self.snippet_action
        } else if bookmark.is_system_tag(SystemTag::Text) {
            &self.text_action
        } else if bookmark.is_system_tag(SystemTag::Shell) {
            &self.shell_action
        } else if bookmark.is_system_tag(SystemTag::Markdown) {
            &self.markdown_action
        } else if bookmark.is_system_tag(SystemTag::Env) {
            &self.env_action
        } else if bookmark.is_system_tag(SystemTag::Memory) {
            &self.memory_action
        } else {
            &self.uri_action
        };
        action.as_ref()
    }
}
