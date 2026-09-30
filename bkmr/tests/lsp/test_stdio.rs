use super::{LspSession, TestDb};
use serde_json::json;

#[test]
fn given_initialize_when_server_starts_then_advertises_completion_and_snippet_commands() {
    let db = TestDb::new();
    let mut lsp = LspSession::start(&db, &[]);

    let result = lsp.initialize();

    assert_eq!(result["serverInfo"]["name"], "bkmr-lsp");
    assert!(result["capabilities"]["completionProvider"].is_object());
    let commands = result["capabilities"]["executeCommandProvider"]["commands"]
        .as_array()
        .expect("advertised commands");
    for command in [
        "bkmr.createSnippet",
        "bkmr.listSnippets",
        "bkmr.getSnippet",
        "bkmr.updateSnippet",
        "bkmr.deleteSnippet",
        "bkmr.insertFilepathComment",
    ] {
        assert!(
            commands.contains(&json!(command)),
            "missing command {}",
            command
        );
    }
    lsp.shutdown();
}

#[test]
fn given_max_debug_logging_when_serving_requests_then_stdout_carries_only_jsonrpc_frames() {
    // Logs must go to stderr: a single stray line on stdout breaks every client.
    let db = TestDb::new();
    db.add("println!(\"{}\", ${1:v});", "rust,_snip_", "Rust println");
    let mut lsp = LspSession::start(&db, &["-ddd"]);
    lsp.initialize();
    lsp.open("file:///tmp/t.rs", "rust", "\n");
    lsp.completion_labels("file:///tmp/t.rs");

    let trailing = lsp.shutdown();

    let invalid: Vec<_> = trailing.iter().filter_map(|f| f.as_ref().err()).collect();
    assert!(invalid.is_empty(), "invalid stdout output: {:?}", invalid);
}

#[test]
fn given_rust_buffer_when_completing_then_offers_only_exactly_tagged_snippets_and_universal() {
    let db = TestDb::new();
    db.add("println!(\"{}\", ${1:v});", "rust,_snip_", "Rust println")
        .add("// TODO: ${1:what}", "universal,_snip_", "Universal TODO")
        .add("https://example.com", "snip,rust", "Not a snippet")
        .add(
            "\"rust-analyzer.check.command\": \"clippy\"",
            "rust-analyzer,_snip_",
            "RA clippy setting",
        );
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.rs", "rust", "\n");

    let labels = lsp.completion_labels("file:///tmp/t.rs");

    assert_eq!(labels, ["Rust println", "Universal TODO"]);
    lsp.shutdown();
}

#[test]
fn given_javascript_buffer_when_completing_then_offers_snippets_tagged_with_any_alias() {
    let db = TestDb::new();
    db.add("console.log(${1:msg})", "js,_snip_", "JS alias log")
        .add(
            "console.error(${1:err})",
            "javascript,_snip_",
            "JS full-name error",
        );
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.js", "javascript", "\n");

    let labels = lsp.completion_labels("file:///tmp/t.js");

    assert_eq!(labels, ["JS alias log", "JS full-name error"]);
    lsp.shutdown();
}

#[test]
fn given_vscode_shellscript_buffer_when_completing_then_offers_bash_tagged_snippet() {
    let db = TestDb::new();
    db.add("echo \"${1:hello}\"", "bash,_snip_", "Bash echo");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.sh", "shellscript", "\n");

    let labels = lsp.completion_labels("file:///tmp/t.sh");

    assert_eq!(labels, ["Bash echo"]);
    lsp.shutdown();
}

#[test]
fn given_hyphenated_language_id_when_completing_then_returns_items_instead_of_failing() {
    let db = TestDb::new();
    db.add(
        "[[Foo alloc] init]",
        "objective-c,_snip_",
        "ObjC alloc init",
    )
    .add("// TODO: ${1:what}", "universal,_snip_", "Universal TODO");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.m", "objective-c", "\n");

    let labels = lsp.completion_labels("file:///tmp/t.m");

    assert_eq!(labels, ["ObjC alloc init", "Universal TODO"]);
    lsp.shutdown();
}

#[test]
fn given_word_before_cursor_when_completing_then_filters_by_title_prefix() {
    let db = TestDb::new();
    db.add("println!(\"{}\", ${1:v});", "rust,_snip_", "Rust println")
        .add("#[derive(Debug)]", "rust,_snip_", "Derive debug");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.rs", "rust", "deri");

    let result = lsp.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": "file:///tmp/t.rs"},
            "position": {"line": 0, "character": 4},
            "context": {"triggerKind": 1}
        }),
    );

    let labels: Vec<&str> = result["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|i| i["label"].as_str().expect("label"))
        .collect();
    assert_eq!(labels, ["Derive debug"]);
    lsp.shutdown();
}

#[test]
fn given_list_snippets_for_javascript_when_executed_then_returns_snippets_of_all_aliases() {
    let db = TestDb::new();
    db.add("console.log(${1:msg})", "js,_snip_", "JS alias log")
        .add(
            "console.error(${1:err})",
            "javascript,_snip_",
            "JS full-name error",
        )
        .add("println!(\"{}\", ${1:v});", "rust,_snip_", "Rust println");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();

    let result = lsp.execute_command("bkmr.listSnippets", json!([{"language": "javascript"}]));

    let mut titles: Vec<&str> = result["snippets"]
        .as_array()
        .expect("snippets")
        .iter()
        .map(|s| s["title"].as_str().expect("title"))
        .collect();
    titles.sort();
    assert_eq!(titles, ["JS alias log", "JS full-name error"]);
    lsp.shutdown();
}

#[test]
fn given_unknown_command_when_executed_then_server_answers_and_stays_alive() {
    let db = TestDb::new();
    db.add("// TODO: ${1:what}", "universal,_snip_", "Universal TODO");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();

    let result = lsp.execute_command("bkmr.doesNotExist", json!([]));
    lsp.open("file:///tmp/t.txt", "plaintext", "\n");
    let labels = lsp.completion_labels("file:///tmp/t.txt");

    assert_eq!(result["success"], false);
    assert_eq!(labels, ["Universal TODO"]);
    lsp.shutdown();
}

#[test]
fn given_fewer_snippets_than_limit_when_completing_then_list_is_complete() {
    // A complete list lets the client filter locally instead of re-querying (and
    // re-rendering every template) on each keystroke.
    let db = TestDb::new();
    db.add("println!(\"{}\", ${1:v});", "rust,_snip_", "Rust println");
    let mut lsp = LspSession::start(&db, &[]);
    lsp.initialize();
    lsp.open("file:///tmp/t.rs", "rust", "\n");

    let result = lsp.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": "file:///tmp/t.rs"},
            "position": {"line": 0, "character": 0},
            "context": {"triggerKind": 1}
        }),
    );

    assert_eq!(result["items"].as_array().expect("items").len(), 1);
    assert_eq!(result["isIncomplete"], false);
    lsp.shutdown();
}
