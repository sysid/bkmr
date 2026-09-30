use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn given_debug_flag_when_running_then_enables_debug_mode() {
    // let config = init_test_env();
    // let _guard = EnvGuard::new();
    let mut cmd = cargo_bin_cmd!("bkmr");
    cmd.args(["-d", "-d"]).assert().success();
    // cmd.args(&["-d", "-d"])
    //     .assert()
    //     .stderr(predicate::str::contains("Debug mode: debug"));
}

/// Runs `bkmr search` against `db` and returns all bookmarks as JSON.
fn search_all(db: &std::path::Path) -> Vec<serde_json::Value> {
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .args(["search", "--np", "--json", ""])
        .output()
        .expect("run bkmr search");
    assert!(output.status.success(), "bkmr search failed");
    serde_json::from_slice(&output.stdout).expect("search --json returns a JSON array")
}

#[test]
fn given_path_when_creating_database_then_creates_empty_database() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("new.db");

    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success()
        .stderr(predicate::str::contains("Database created"));

    assert!(db.exists());
    assert_eq!(search_all(&db), Vec::<serde_json::Value>::new());
}

#[test]
fn given_pre_fill_when_creating_database_then_contains_only_demo_entries() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("prefilled.db");

    cargo_bin_cmd!("bkmr")
        .args(["create-db", "--pre-fill"])
        .arg(&db)
        .assert()
        .success();

    // pre_fill_database (cli/bookmark_commands.rs) defines 20 demo entries
    let bookmarks = search_all(&db);
    assert_eq!(bookmarks.len(), 20);
    assert!(bookmarks.iter().any(|b| b["url"] == "https://github.com"));
}

#[test]
fn given_bookmark_ids_when_showing_then_displays_correct_entries() {
    // Arrange: three bookmarks; show only the first and the third
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("show.db");
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();
    for (url, title) in [
        ("https://first.example.com", "First Shown"),
        ("https://second.example.com", "Second Hidden"),
        ("https://third.example.com", "Third Shown"),
    ] {
        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args([
                "add",
                url,
                "show-test",
                "--title",
                title,
                "--no-web",
                "--no-embed",
            ])
            .assert()
            .success();
    }
    let id_of = |title: &str| {
        search_all(&db)
            .iter()
            .find(|b| b["title"] == title)
            .map(|b| b["id"].to_string())
            .expect("bookmark added")
    };
    let ids = format!("{},{}", id_of("First Shown"), id_of("Third Shown"));

    // Act + Assert: JSON output contains exactly the requested bookmarks
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", "--json", &ids])
        .output()
        .expect("run bkmr show --json");
    assert!(output.status.success());
    let shown: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut titles: Vec<&str> = shown.iter().map(|b| b["title"].as_str().unwrap()).collect();
    titles.sort();
    assert_eq!(titles, ["First Shown", "Third Shown"]);

    // Act + Assert: detailed output shows the same entries, not the unrequested one
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &ids])
        .assert()
        .success()
        .stdout(predicate::str::contains("First Shown"))
        .stdout(predicate::str::contains("https://third.example.com"))
        .stdout(predicate::str::contains("Second Hidden").not());
}
