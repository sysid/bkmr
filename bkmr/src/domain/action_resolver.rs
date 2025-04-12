// src/domain/action_resolver.rs
use crate::domain::action::BookmarkAction;
use crate::domain::bookmark::Bookmark;
use crate::domain::error::{DomainError, DomainResult};
use crate::domain::system_tag::SystemTag;
use std::fmt::Debug;

/// Resolves the appropriate action for a bookmark based on its tags or other properties
pub trait ActionResolver: Debug + Send + Sync {
    /// Resolves the default action for a bookmark
    fn resolve_action(&self, bookmark: &Bookmark) -> Box<dyn BookmarkAction + '_>;
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
    default_action: Box<dyn BookmarkAction>,
}

impl SystemTagActionResolver {
    pub fn new(
        uri_action: Box<dyn BookmarkAction>,
        snippet_action: Box<dyn BookmarkAction>,
        text_action: Box<dyn BookmarkAction>,
        shell_action: Box<dyn BookmarkAction>,
        markdown_action: Box<dyn BookmarkAction>,
        env_action: Box<dyn BookmarkAction>,
        default_action: Box<dyn BookmarkAction>,
    ) -> Self {
        Self {
            uri_action,
            snippet_action,
            text_action,
            shell_action,
            markdown_action,
            env_action,
            default_action,
        }
    }
}

impl ActionResolver for SystemTagActionResolver {
    fn resolve_action(&self, bookmark: &Bookmark) -> Box<dyn BookmarkAction + '_> {
        if bookmark.is_snippet() {
            Box::new(SnippetActionProxy(self.snippet_action.as_ref()))
        } else if bookmark.is_system_tag(SystemTag::Text) {
            Box::new(TextActionProxy(self.text_action.as_ref()))
        } else if bookmark.is_system_tag(SystemTag::Shell) {
            Box::new(ShellActionProxy(self.shell_action.as_ref()))
        } else if bookmark.is_system_tag(SystemTag::Markdown) {
            Box::new(MarkdownActionProxy(self.markdown_action.as_ref()))
        } else if bookmark.is_system_tag(SystemTag::Env) {
            Box::new(EnvActionProxy(self.env_action.as_ref()))
        } else if bookmark.is_uri() {
            Box::new(UriActionProxy(self.uri_action.as_ref()))
        } else {
            Box::new(DefaultActionProxy(self.default_action.as_ref()))
        }
    }
}

// Proxy types to wrap the boxed actions for returning
#[derive(Debug)]
struct SnippetActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct TextActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct ShellActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct MarkdownActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct EnvActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct UriActionProxy<'a>(&'a dyn BookmarkAction);
#[derive(Debug)]
struct DefaultActionProxy<'a>(&'a dyn BookmarkAction);

impl<'a> BookmarkAction for SnippetActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for TextActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for ShellActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for MarkdownActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for EnvActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for UriActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}

impl<'a> BookmarkAction for DefaultActionProxy<'a> {
    fn execute(&self, bookmark: &Bookmark) -> DomainResult<()> {
        self.0.execute(bookmark)
    }

    fn description(&self) -> &'static str {
        self.0.description()
    }
}
