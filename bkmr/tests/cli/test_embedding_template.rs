//! The EMBEDDABLE template section: what the editor shows is what gets stored.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use bkmr::application::services::template_service::{TemplateService, TemplateServiceImpl};
use bkmr::application::services::BookmarkService;
use bkmr::cli::args::Cli;
use bkmr::cli::bookmark_commands::add;
use bkmr::cli::process::{clone_bookmark, edit_bookmarks};
use bkmr::config::Settings;
use bkmr::domain::bookmark::Bookmark;
use bkmr::util::test_context::TestContext;
use clap::Parser;
use tempfile::TempDir;

/// Editor stub that records the template it was shown and then applies `sed` expressions.
/// Sleeps because bkmr detects a save through the file's modification time.
fn editor_applying(dir: &TempDir, sed_expressions: &[&str]) -> Arc<dyn TemplateService> {
    let script = dir.path().join("editor.sh");
    let shown = dir.path().join("shown.txt");
    let expressions: String = sed_expressions
        .iter()
        .map(|e| format!(" -e '{}'", e))
        .collect();
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncp \"$1\" '{shown}'\nsleep 0.05\nsed -e '' {expressions} \"$1\" > \"$1.new\"\ncat \"$1.new\" > \"$1\"\n",
            shown = shown.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    Arc::new(TemplateServiceImpl::with_editor(
        script.to_string_lossy().to_string(),
    ))
}

/// The `sed` expression replacing the line below the EMBEDDABLE marker.
fn set_embeddable(value: &str) -> String {
    format!("/=== EMBEDDABLE ===/{{n;s/.*/{}/;}}", value)
}

fn shown_embeddable(dir: &TempDir) -> String {
    section_value(&dir.path().join("shown.txt"), "=== EMBEDDABLE ===")
}

fn section_value(template: &Path, marker: &str) -> String {
    let text = std::fs::read_to_string(template).expect("editor was opened");
    let mut lines = text.lines();
    lines.find(|line| *line == marker).expect("marker present");
    lines.next().expect("line below marker").to_string()
}

fn stored_by_url(service: &Arc<dyn BookmarkService>, url: &str) -> Bookmark {
    service
        .get_all_bookmarks(None, None)
        .unwrap()
        .into_iter()
        .find(|b| b.url == url)
        .expect("bookmark stored")
}

fn run_add(
    args: &[&str],
    service: &Arc<dyn BookmarkService>,
    editor: Arc<dyn TemplateService>,
) {
    let mut argv = vec!["bkmr", "add"];
    argv.extend_from_slice(args);
    add(Cli::parse_from(argv), service.clone(), editor).expect("add succeeds");
}

fn add_non_embeddable(service: &Arc<dyn BookmarkService>, url: &str) -> i32 {
    service
        .add_bookmark(url, Some("Source"), None, None, false, false, None)
        .unwrap()
        .id
        .unwrap()
}

#[test]
fn given_no_flag_when_add_edit_then_template_shows_and_stores_true() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &[]);

    // Act
    run_add(&["echo plain", "--edit", "-t", "snip"], &service, editor);

    // Assert
    assert_eq!(shown_embeddable(&dir), "true");
    assert!(stored_by_url(&service, "echo plain").embeddable);
}

#[test]
fn given_no_embed_flag_when_add_edit_then_template_shows_and_stores_false() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &[]);

    // Act
    run_add(
        &["echo flagged", "--edit", "--no-embed", "-t", "snip"],
        &service,
        editor,
    );

    // Assert
    assert_eq!(shown_embeddable(&dir), "false");
    assert!(!stored_by_url(&service, "echo flagged").embeddable);
}

#[test]
fn given_user_sets_false_in_template_when_add_edit_then_stores_false() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &[&set_embeddable("false")]);

    // Act
    run_add(&["echo off", "--edit", "-t", "shell"], &service, editor);

    // Assert
    assert!(!stored_by_url(&service, "echo off").embeddable);
}

#[test]
fn given_no_embed_flag_and_user_sets_true_in_template_when_add_edit_then_saved_value_wins() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &[&set_embeddable("true")]);

    // Act
    run_add(
        &["echo wins", "--edit", "--no-embed", "-t", "snip"],
        &service,
        editor,
    );

    // Assert
    assert_eq!(shown_embeddable(&dir), "false");
    assert!(stored_by_url(&service, "echo wins").embeddable);
}

#[test]
fn given_non_embeddable_source_when_add_clone_then_clone_stays_non_embeddable() {
    // Arrange
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let id = add_non_embeddable(&service, "clone source");
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &["/=== URL ===/{n;s/.*/add clone/;}"]);

    // Act
    run_add(&["-c", &id.to_string()], &service, editor);

    // Assert
    assert_eq!(shown_embeddable(&dir), "false");
    assert!(!stored_by_url(&service, "add clone").embeddable);
}

#[test]
fn given_non_embeddable_source_when_clone_bookmark_then_clone_stays_non_embeddable() {
    // Arrange: the clone action of the fzf interface
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let id = add_non_embeddable(&service, "fzf clone source");
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(&dir, &["/=== URL ===/{n;s/.*/fzf clone/;}"]);

    // Act
    clone_bookmark(id, service.clone(), editor).unwrap();

    // Assert
    assert!(!stored_by_url(&service, "fzf clone").embeddable);
}

#[test]
fn given_id_removed_in_template_when_edit_then_new_bookmark_keeps_saved_flag() {
    // Arrange: removing the ID turns the edit into a new bookmark
    let ctx = TestContext::new();
    let service = ctx.bookmark_service();
    let id = add_non_embeddable(&service, "edit source");
    let dir = TempDir::new().unwrap();
    let editor = editor_applying(
        &dir,
        &[
            "/=== ID ===/{n;s/.*//;}",
            "/=== URL ===/{n;s/.*/edit as new/;}",
        ],
    );

    // Act
    edit_bookmarks(vec![id], true, service.clone(), editor, &Settings::default()).unwrap();

    // Assert
    assert!(!stored_by_url(&service, "edit as new").embeddable);
}
