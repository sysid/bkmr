use bkmr::domain::bookmark::{Bookmark, BookmarkBuilder};
use bkmr::domain::interpolation::errors::InterpolationError;
use bkmr::domain::interpolation::interface::{InterpolationEngine, ShellCommandExecutor};
use bkmr::domain::tag::Tag;
use bkmr::infrastructure::interpolation::minijinja_engine::MiniJinjaEngine;
use bkmr::util::testing::init_test_env;
use chrono::{Datelike, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// Mock shell executor for testing
#[derive(Clone, Debug)]
struct MockShellExecutor {
    responses: HashMap<String, String>,
}

impl MockShellExecutor {
    fn new() -> Self {
        let mut responses = HashMap::new();
        responses.insert("date".to_string(), "2023-01-01".to_string());
        responses.insert("whoami".to_string(), "testuser".to_string());
        Self { responses }
    }
}

impl ShellCommandExecutor for MockShellExecutor {
    fn execute(&self, cmd: &str) -> Result<String, InterpolationError> {
        match self.responses.get(cmd) {
            Some(response) => Ok(response.clone()),
            None => Err(InterpolationError::Shell(format!(
                "Unknown command: {}",
                cmd
            ))),
        }
    }

    fn arc_clone(&self) -> Arc<dyn ShellCommandExecutor> {
        Arc::new(self.clone())
    }
}

fn create_test_bookmark() -> Bookmark {
    let mut tags = HashSet::new();
    tags.insert(Tag::new("test").unwrap());
    tags.insert(Tag::new("example").unwrap());

    BookmarkBuilder::default()
        .id(Some(42))
        .url("https://example.com/{{ env_USER }}/{{ title | lower }}")
        .title("Test Bookmark")
        .description("A test bookmark")
        .tags(tags)
        .access_count(5)
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .embedding(None)
        .content_hash(None)
        .embeddable(false)
        .build()
        .unwrap()
}

#[test]
fn test_render_static_url() {
    let _test_env = init_test_env();
    let engine = MiniJinjaEngine::new(Arc::new(MockShellExecutor::new()));
    let url = "https://example.com";

    let result = engine.render_url(url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), url);
}

#[test]
fn test_render_template_url() {
    let _test_env = init_test_env();
    std::env::set_var("USER", "testuser");

    let engine = MiniJinjaEngine::new(Arc::new(MockShellExecutor::new()));
    let url = "https://example.com/{{ env_USER }}/profile";

    let result = engine.render_url(url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://example.com/testuser/profile");
}

#[test]
fn test_render_bookmark_url() {
    let _test_env = init_test_env();
    std::env::set_var("USER", "testuser");

    let engine = MiniJinjaEngine::new(Arc::new(MockShellExecutor::new()));
    let bookmark = create_test_bookmark();

    let result = engine.render_bookmark_url(&bookmark);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap(),
        "https://example.com/testuser/test bookmark"
    );
}

#[test]
fn test_date_filters() {
    let _test_env = init_test_env();
    let engine = MiniJinjaEngine::new(Arc::new(MockShellExecutor::new()));

    // Use current_date which is added to context
    let url = "https://example.com/{{ current_date | strftime('%Y-%m') }}";
    let today = Utc::now();
    let expected = format!("https://example.com/{}-{:02}", today.year(), today.month());

    let result = engine.render_url(url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_shell_filter() {
    let _test_env = init_test_env();
    let engine = MiniJinjaEngine::new(Arc::new(MockShellExecutor::new()));

    let url = "https://example.com/{{ 'whoami' | shell }}";

    let result = engine.render_url(url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "https://example.com/testuser");
}
