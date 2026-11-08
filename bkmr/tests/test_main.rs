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

#[test]
fn given_repeated_show_when_showing_then_reports_state_before_current_access() {
    // `show` records an access itself; reporting the state after that would make
    // "last accessed" always read "now".
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("show-access.db");
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args([
            "add",
            "https://example.com",
            "show-test",
            "--title",
            "Example",
            "--no-web",
            "--no-embed",
        ])
        .assert()
        .success();
    let id = search_all(&db)[0]["id"].to_string();

    // First show: never accessed before
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Last accessed: never"))
        .stdout(predicate::str::contains("Access count: 0"));

    // Second show: reports the first show
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Last accessed: never").not())
        .stdout(predicate::str::contains("Access count: 1"));

    // JSON output follows the same rule: two shows so far
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", "--json", &id])
        .output()
        .expect("run bkmr show --json");
    assert!(output.status.success());
    let shown: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(shown[0]["access_count"], 2);
}

/// Creates a database at `db` holding one bookmark without an embedding.
fn create_db_with_one_bookmark(db: &std::path::Path) {
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(db)
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .args([
            "add",
            "https://example.com",
            "show-test",
            "--title",
            "Example",
            "--no-web",
            "--no-embed",
        ])
        .assert()
        .success();
}

#[test]
fn given_piped_stdout_when_showing_then_writes_no_color_codes() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("show-color.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    // assert_cmd captures stdout through a pipe
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .env_remove("NO_COLOR")
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\u{1b}").not())
        .stdout(predicate::str::contains("  Description: "));
}

#[test]
fn given_bookmark_when_show_or_search_json_then_includes_all_metadata() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("show-json.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", "--json", &id])
        .output()
        .expect("run bkmr show --json");
    assert!(output.status.success());
    let shown: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();

    for bookmark in [&shown[0], &search_all(&db)[0]] {
        assert_eq!(bookmark["embeddable"], false);
        assert_eq!(bookmark["has_embedding"], false);
        for key in [
            "opener",
            "content_hash",
            "file_path",
            "file_mtime",
            "file_hash",
        ] {
            assert!(bookmark.get(key).is_some(), "missing JSON field: {}", key);
        }
    }
}

/// Reads `has_embedding` of bookmark `id` through `show --json`.
fn has_embedding(db: &std::path::Path, id: i64) -> bool {
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .args(["show", "--json", &id.to_string()])
        .output()
        .expect("run bkmr show --json");
    assert!(output.status.success(), "bkmr show failed");
    let shown: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    shown[0]["has_embedding"]
        .as_bool()
        .expect("embedding lookup succeeds")
}

#[test]
fn given_stored_vector_when_backfill_force_dry_run_then_keeps_vector() {
    use bkmr::domain::repositories::vector_repository::VectorRepository;
    use bkmr::infrastructure::repositories::sqlite::register_sqlite_vec;
    use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;

    // Arrange: one bookmark whose vector is written directly, so the model is not needed
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("backfill.db");
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args([
            "add",
            "https://backfill.example.com",
            "t",
            "--title",
            "Backfill",
        ])
        .args(["--no-web", "--no-embed"])
        .assert()
        .success();
    let id = search_all(&db)[0]["id"].as_i64().expect("bookmark added");
    register_sqlite_vec();
    let vectors = SqliteVectorRepository::new(db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(768).unwrap();
    vectors
        .upsert_embedding(id as i32, &vec![0.1; 768])
        .unwrap();
    drop(vectors);
    assert!(has_embedding(&db, id));

    // Act
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["backfill", "--force", "--dry-run"])
        .assert()
        .success();

    // Assert: a dry run changes nothing
    assert!(has_embedding(&db, id), "dry run deleted the vector");
}

#[test]
fn given_vector_on_non_embeddable_bookmark_when_backfill_prune_then_removes_only_that_vector() {
    use bkmr::domain::repositories::repository::BookmarkRepository;
    use bkmr::domain::repositories::vector_repository::VectorRepository;
    use bkmr::infrastructure::repositories::sqlite::register_sqlite_vec;
    use bkmr::infrastructure::repositories::sqlite::repository::SqliteBookmarkRepository;
    use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;

    // Arrange: vectors are written directly, so the model is not needed
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("prune.db");
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();
    for (url, title) in [
        ("https://stale.example.com", "Stale"),
        ("https://keep.example.com", "Keep"),
    ] {
        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(["add", url, "t", "--title", title, "--no-web", "--no-embed"])
            .assert()
            .success();
    }
    let id_of = |title: &str| {
        search_all(&db)
            .iter()
            .find(|b| b["title"] == title)
            .and_then(|b| b["id"].as_i64())
            .expect("bookmark added")
    };
    let (stale, keep) = (id_of("Stale"), id_of("Keep"));

    let bookmarks = SqliteBookmarkRepository::from_url(db.to_str().unwrap()).unwrap();
    let mut embeddable = bookmarks.get_by_id(keep as i32).unwrap().unwrap();
    embeddable.set_embeddable(true);
    bookmarks.update(&embeddable).unwrap();

    register_sqlite_vec();
    let vectors = SqliteVectorRepository::new(db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(768).unwrap();
    for id in [stale, keep] {
        vectors
            .upsert_embedding(id as i32, &vec![0.1; 768])
            .unwrap();
    }
    drop(vectors);

    // Act + Assert: a dry run reports the vector and changes nothing
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["backfill", "--prune", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains(format!("ID: {stale}")));
    assert!(has_embedding(&db, stale), "dry run deleted the vector");

    // Act
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["backfill", "--prune"])
        .assert()
        .success();

    // Assert
    assert!(!has_embedding(&db, stale), "stale vector must be removed");
    assert!(has_embedding(&db, keep), "embeddable vector must stay");
}

#[test]
fn given_prune_and_force_when_running_backfill_then_rejected() {
    // Force deletes every vector, which would contradict removing only the stale ones
    cargo_bin_cmd!("bkmr")
        .args(["backfill", "--prune", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn given_backfill_help_when_shown_then_force_does_not_claim_to_skip_imported() {
    // get_bookmarks_for_forced_backfill filters on the embeddable flag only
    cargo_bin_cmd!("bkmr")
        .args(["backfill", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_imported_").not());
}

#[test]
fn given_invalid_default_config_when_running_then_fails_instead_of_using_another_db() {
    // Arrange: a usable database in the default location, the one a silent fallback would open
    let home = TempDir::new().unwrap();
    let config_dir = home.path().join(".config/bkmr");
    std::fs::create_dir_all(&config_dir).unwrap();
    cargo_bin_cmd!("bkmr")
        .env("HOME", home.path())
        .arg("create-db")
        .arg(config_dir.join("bkmr.db"))
        .assert()
        .success();
    std::fs::write(config_dir.join("config.toml"), "[fzf_opts]\nheight = 5\n").unwrap();

    // Act + Assert
    cargo_bin_cmd!("bkmr")
        .env("HOME", home.path())
        .env_remove("BKMR_DB_URL")
        .args(["search", "--np", "--json", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("config.toml"));
}

#[test]
fn given_invalid_default_config_when_generating_config_then_still_succeeds() {
    // Arrange
    let home = TempDir::new().unwrap();
    let config_dir = home.path().join(".config/bkmr");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), "[fzf_opts]\nheight = 5\n").unwrap();

    // Act + Assert: generating a fresh config is how users recover from a broken one
    cargo_bin_cmd!("bkmr")
        .env("HOME", home.path())
        .arg("--generate-config")
        .assert()
        .success();
}

#[test]
fn given_invalid_config_when_starting_lsp_then_exits_with_plain_error_for_client_logs() {
    // Arrange: editors capture the server's stderr into their LSP log, which shows raw bytes
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("broken.toml");
    std::fs::write(&config, "[embeddings.policy]\n_snip_ = \"offf\"\n").unwrap();

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env_remove("NO_COLOR")
        .arg("--config")
        .arg(&config)
        .arg("lsp")
        .output()
        .expect("run bkmr lsp");

    // Assert
    assert_eq!(output.status.code(), Some(64), "expected EX_USAGE");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("broken.toml"), "got: {stderr}");
    assert!(stderr.contains("offf"), "got: {stderr}");
    assert!(!stderr.contains('\x1b'), "ANSI escape in: {stderr:?}");
}

#[test]
fn given_no_color_flag_when_config_is_invalid_then_error_has_no_ansi_escapes() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("broken.toml");
    std::fs::write(&config, "[embeddings.policy]\n_snip_ = \"offf\"\n").unwrap();

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env_remove("NO_COLOR")
        .arg("--no-color")
        .arg("--config")
        .arg(&config)
        .args(["search", "--np", "--json", ""])
        .output()
        .expect("run bkmr search");

    // Assert
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains('\x1b'), "ANSI escape in: {stderr:?}");
}

#[test]
fn given_config_with_snippets_off_when_adding_through_binary_then_not_embedded() {
    // Arrange: no flag and no model, so a wrongly embedded snippet would load the model
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("policy.db");
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[embeddings.policy]\n_snip_ = \"off\"\n").unwrap();
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();

    // Act
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .arg("--config")
        .arg(&config)
        .args(["add", "echo hi", "t", "--title", "Hi", "-t", "snip"])
        .assert()
        .success();

    // Assert
    let bookmark = &search_all(&db)[0];
    assert_eq!(bookmark["embeddable"], false);
    assert_eq!(bookmark["has_embedding"], false);
}

#[test]
fn given_config_with_policy_when_showing_info_then_prints_effective_policy() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("info.db");
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[embeddings.policy]\n_snip_ = \"off\"\n").unwrap();
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();

    // Act + Assert
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .arg("--config")
        .arg(&config)
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("_snip_=off"))
        .stdout(predicate::str::contains("_mem_=always"));
}

#[test]
fn given_invalid_policy_when_running_then_fails_naming_the_value() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[embeddings.policy]\n_snip_ = \"never\"\n").unwrap();

    // Act + Assert
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", dir.path().join("unused.db"))
        .arg("--config")
        .arg(&config)
        .arg("info")
        .assert()
        .failure()
        .stderr(predicate::str::contains("never"));
}

// Exit codes are part of the CLI contract for scripts (see ERROR_HANDLING.md).

#[test]
fn given_unknown_flag_when_running_then_exits_with_usage_code() {
    // Act
    let output = cargo_bin_cmd!("bkmr")
        .arg("--no-such-flag")
        .output()
        .expect("run bkmr");

    // Assert
    assert_eq!(output.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--no-such-flag"));
}

#[test]
fn given_no_arguments_when_running_then_exits_with_usage_code() {
    // Act
    let output = cargo_bin_cmd!("bkmr").output().expect("run bkmr");

    // Assert
    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn given_help_flag_when_running_then_exits_successfully_with_help_on_stdout() {
    // Act
    let output = cargo_bin_cmd!("bkmr")
        .arg("--help")
        .output()
        .expect("run bkmr");

    // Assert
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

#[test]
fn given_non_numeric_id_when_showing_then_exits_with_usage_code() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("exit.db");
    create_db_with_one_bookmark(&db);

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", "not-a-number"])
        .output()
        .expect("run bkmr show");

    // Assert
    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn given_unsupported_shell_when_generating_completion_then_exits_with_usage_code() {
    // Act
    let output = cargo_bin_cmd!("bkmr")
        .args(["completion", "no-such-shell"])
        .output()
        .expect("run bkmr completion");

    // Assert
    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn given_missing_database_when_searching_then_exits_with_failure_code() {
    // Arrange
    let dir = TempDir::new().unwrap();

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", dir.path().join("missing.db"))
        .args(["search", "--np", "--json", ""])
        .output()
        .expect("run bkmr search");

    // Assert
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn given_unknown_clone_id_when_adding_then_exits_with_failure_code() {
    // Arrange: the arguments are well formed, the failure comes from the database content
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("exit.db");
    create_db_with_one_bookmark(&db);

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["add", "--clone", "99999"])
        .output()
        .expect("run bkmr add --clone");

    // Assert
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("No bookmark found with ID 99999"));
}

#[test]
fn given_missing_path_when_importing_files_then_exits_with_failure_code() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("exit.db");
    create_db_with_one_bookmark(&db);

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .arg("import-files")
        .arg(dir.path().join("no-such-directory"))
        .output()
        .expect("run bkmr import-files");

    // Assert
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn given_duplicate_name_without_update_when_importing_files_then_exits_with_duplicate_code() {
    // Arrange
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("exit.db");
    create_db_with_one_bookmark(&db);
    let script = dir.path().join("script.sh");
    std::fs::write(
        &script,
        "#!/bin/bash\n# name: dup-script\n# tags: t\necho hi\n",
    )
    .unwrap();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["import-files", "--no-embed"])
        .arg(&script)
        .assert()
        .success();

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["import-files", "--no-embed"])
        .arg(&script)
        .output()
        .expect("run bkmr import-files");

    // Assert
    assert_eq!(output.status.code(), Some(65));
}

#[test]
fn given_declined_prompt_when_clearing_embeddings_then_exits_with_cancel_code_and_keeps_vector() {
    use bkmr::domain::repositories::vector_repository::VectorRepository;
    use bkmr::infrastructure::repositories::sqlite::register_sqlite_vec;
    use bkmr::infrastructure::repositories::sqlite::vector_repository::SqliteVectorRepository;

    // Arrange: the vector is written directly, so the model is not needed
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("exit.db");
    create_db_with_one_bookmark(&db);
    let bookmark_id = search_all(&db)[0]["id"].as_i64().expect("bookmark id");
    register_sqlite_vec();
    let vectors = SqliteVectorRepository::new(db.to_str().unwrap()).unwrap();
    vectors.init_vec_table(768).unwrap();
    vectors
        .upsert_embedding(bookmark_id as i32, &vec![0.1; 768])
        .unwrap();
    drop(vectors);

    // Act
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .arg("clear-embeddings")
        .write_stdin("n\n")
        .output()
        .expect("run bkmr clear-embeddings");

    // Assert
    assert_eq!(output.status.code(), Some(130));
    assert!(
        has_embedding(&db, bookmark_id),
        "declined clear deleted the vector"
    );
}

fn access_count_of_first_bookmark(db: &std::path::Path) -> i64 {
    search_all(db)[0]["access_count"].as_i64().unwrap()
}

#[test]
fn given_open_stdout_when_running_then_prints_content_and_records_access() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("access.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["open", "--stdout", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://example.com"));

    assert_eq!(access_count_of_first_bookmark(&db), 1);
}

#[test]
fn given_stdout_without_fzf_when_hsearch_then_keeps_table_and_does_not_record_access() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("hsearch-stdout.db");
    create_db_with_one_bookmark(&db);

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["hsearch", "example", "--mode", "exact", "--stdout", "--np"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\t"));

    assert_eq!(access_count_of_first_bookmark(&db), 0);
}

#[test]
fn given_json_with_stdout_when_hsearch_then_keeps_json_output() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("hsearch-json-stdout.db");
    create_db_with_one_bookmark(&db);

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args([
            "hsearch", "example", "--mode", "exact", "--stdout", "--json", "--np",
        ])
        .output()
        .expect("run bkmr hsearch --stdout --json");
    assert!(output.status.success());

    let hits: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("stdout stays valid JSON");
    assert_eq!(hits[0]["url"], "https://example.com");
    assert_eq!(access_count_of_first_bookmark(&db), 0);
}

#[test]
fn given_result_lists_when_running_then_does_not_record_access() {
    // Nobody picked a specific bookmark, so listing search results is not an access.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("access.db");
    create_db_with_one_bookmark(&db);

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["search", "--np", "example"])
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["search", "--np", "--json", "example"])
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["hsearch", "--np", "--json", "--mode", "exact", "example"])
        .assert()
        .success();

    assert_eq!(access_count_of_first_bookmark(&db), 0);
}

fn make_bookmark_writes_fail(db: &std::path::Path) {
    rusqlite::Connection::open(db)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_access_write BEFORE UPDATE ON bookmarks \
             BEGIN SELECT RAISE(ABORT, 'write refused'); END",
        )
        .unwrap();
}

#[test]
fn given_failing_access_write_when_open_stdout_then_still_delivers_content_and_warns() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("access.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    make_bookmark_writes_fail(&db);

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["open", "--stdout", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://example.com"))
        .stderr(predicate::str::contains("Failed to record access"));
}

#[test]
fn given_failing_access_write_when_show_then_still_shows_details_and_warns() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("access.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    make_bookmark_writes_fail(&db);

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://example.com"))
        .stderr(predicate::str::contains("Failed to record access"));
}

fn add_second_bookmark(db: &std::path::Path) {
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .args([
            "add",
            "https://example.org",
            "show-test",
            "--title",
            "Second",
            "--no-web",
            "--no-embed",
        ])
        .assert()
        .success();
}

#[test]
fn given_failing_delete_when_delete_then_exits_failure_and_reports_error() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_delete BEFORE DELETE ON bookmarks \
             BEGIN SELECT RAISE(ABORT, 'delete refused'); END",
        )
        .unwrap();

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &id])
        .write_stdin("y\n")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error deleting bookmark"));
    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_unknown_id_among_ids_when_delete_then_deletes_the_rest_and_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &format!("99999,{id}")])
        .write_stdin("y\n")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("99999"));
    assert_eq!(search_all(&db).len(), 0, "the valid ID was not deleted");
}

#[test]
fn given_declined_prompt_when_delete_then_keeps_bookmark_and_exits_cancel() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &id])
        .write_stdin("n\n")
        .assert()
        .code(130)
        .stderr(predicate::str::contains("Deletion cancelled"));

    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_no_input_when_delete_then_keeps_bookmark_and_exits_cancel() {
    // An agent has no stdin. Exit 0 here would read as the bookmark being gone.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &id])
        .write_stdin("")
        .assert()
        .code(130);

    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_yes_when_delete_then_deletes_all_without_asking() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    add_second_bookmark(&db);
    let ids: Vec<String> = search_all(&db)
        .iter()
        .map(|b| b["id"].to_string())
        .collect();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", "--yes", &ids.join(",")])
        .write_stdin("")
        .assert()
        .success()
        .stdout(predicate::str::contains("Confirm").not());

    assert_eq!(search_all(&db).len(), 0);
}

#[test]
fn given_yes_and_unknown_id_when_delete_then_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", "-y", "9999"])
        .write_stdin("")
        .assert()
        .code(1);

    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_yes_and_force_when_delete_then_rejected_and_nothing_is_removed() {
    // A purge cannot be undone, so skipping its prompt must take more than one flag.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", "--yes", "--force", &id])
        .write_stdin("")
        .assert()
        .code(64);

    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_memory_when_deleted_with_yes_then_it_can_be_restored() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    let output = run_with_config(&db, &config, &["delete", "--yes", &id], "");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(search_all(&db).len(), 0);
    assert_eq!(history_of(&db, &[&id]).len(), 1);
}

#[test]
fn given_two_ids_when_delete_confirmed_then_deletes_both_and_exits_success() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    add_second_bookmark(&db);
    let ids: Vec<String> = search_all(&db)
        .iter()
        .map(|b| b["id"].to_string())
        .collect();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &ids.join(",")])
        .write_stdin("y\ny\n")
        .assert()
        .success();

    assert_eq!(search_all(&db).len(), 0);
}

fn verified_at(db: &std::path::Path, id: &str) -> serde_json::Value {
    search_all(db)
        .into_iter()
        .find(|bookmark| bookmark["id"].to_string() == id)
        .expect("bookmark is in the search results")["verified_at"]
        .clone()
}

#[test]
fn given_ids_separated_by_spaces_or_commas_when_verify_then_all_are_stamped() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    add_second_bookmark(&db);
    let ids: Vec<String> = search_all(&db)
        .iter()
        .map(|b| b["id"].to_string())
        .collect();
    assert!(verified_at(&db, &ids[0]).is_null());

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &ids[0], &ids[1]])
        .assert()
        .success();

    assert!(verified_at(&db, &ids[0]).is_string());
    assert!(verified_at(&db, &ids[1]).is_string());

    let before = verified_at(&db, &ids[0]);
    std::thread::sleep(std::time::Duration::from_millis(5));
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &ids.join(",")])
        .assert()
        .success();
    assert_ne!(verified_at(&db, &ids[0]), before);
}

#[test]
fn given_unknown_id_among_ids_when_verify_then_stamps_the_rest_and_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", "9999", &id])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("9999"));

    assert!(verified_at(&db, &id).is_string());
}

#[test]
fn given_no_id_or_a_word_when_verify_then_usage_error() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);

    for args in [vec!["verify"], vec!["verify", "abc"]] {
        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(args)
            .assert()
            .code(64);
    }
}

#[test]
fn given_verify_when_run_then_updated_at_and_access_count_stay() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let before = search_all(&db)[0].clone();
    let id = before["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &id])
        .assert()
        .success();

    let after = search_all(&db)[0].clone();
    assert_eq!(after["updated_at"], before["updated_at"]);
    assert_eq!(after["access_count"], before["access_count"]);
}

#[test]
fn given_verified_bookmark_when_text_stays_then_the_stamp_stays() {
    // Bulk rewrites reset updated_at before: a write that leaves the text alone must not move this.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &id])
        .assert()
        .success();
    let stamp = verified_at(&db, &id);

    for args in [
        vec!["update", "-d", "why", &id],
        vec!["update", "-t", "extra", &id],
        vec!["update", "--url", "https://example.com", &id],
        vec!["show", &id],
    ] {
        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(args)
            .assert()
            .success();
    }

    assert_eq!(verified_at(&db, &id), stamp);
}

#[test]
fn given_verified_bookmark_when_url_or_title_changes_then_the_stamp_is_cleared() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    for args in [
        vec!["update", "--url", "https://example.com/new", &id],
        vec!["update", "--title", "New title", &id],
    ] {
        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(["verify", &id])
            .assert()
            .success();
        assert!(verified_at(&db, &id).is_string());

        cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(args)
            .assert()
            .success();

        assert!(verified_at(&db, &id).is_null());
    }
}

#[test]
fn given_corrected_then_verified_memory_when_old_state_is_restored_then_the_stamp_is_cleared() {
    // The restored text is what the check disproved, so it must not carry the check's stamp.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["update", "--url", "port 5433", &id], "");
    run_with_config(&db, &config, &["verify", &id], "");
    assert!(verified_at(&db, &id).is_string());
    let history_id = history_of(&db, &[&id])[0]["history_id"].to_string();

    let restore = run_with_config(&db, &config, &["restore", &history_id], "");

    assert!(restore.status.success());
    assert!(verified_at(&db, &id).is_null());
}

#[test]
fn given_repeated_ids_when_verify_then_each_is_handled_once() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    let known = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &id, &id])
        .output()
        .expect("run bkmr verify");
    assert!(known.status.success());
    let stderr = String::from_utf8_lossy(&known.stderr);
    assert_eq!(stderr.matches("Verified bookmark").count(), 1, "{stderr}");

    let unknown = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", "9999,9999"])
        .output()
        .expect("run bkmr verify");
    assert!(!unknown.status.success());
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("IDs: [9999]"), "{stderr}");
}

#[test]
fn given_memory_when_verified_then_no_undo_step_is_written_and_a_restored_copy_is_unverified() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    run_with_config(&db, &config, &["verify", &id], "");
    assert!(history_of(&db, &[&id]).is_empty());

    run_with_config(&db, &config, &["delete", "--yes", &id], "");
    let history_id = history_of(&db, &[&id])[0]["history_id"].to_string();
    run_with_config(&db, &config, &["restore", &history_id], "");

    assert!(verified_at(&db, &id).is_null());
}

#[test]
fn given_bookmark_when_hsearch_json_then_age_fields_are_included() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    let hsearch = || -> Vec<serde_json::Value> {
        let output = cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(["hsearch", "Example", "--mode", "exact", "--json", "--np"])
            .output()
            .expect("run bkmr hsearch");
        assert!(output.status.success());
        serde_json::from_slice(&output.stdout).expect("hsearch --json returns a JSON array")
    };

    let before = &hsearch()[0];
    assert!(before["created_at"].is_string());
    assert!(before["verified_at"].is_null());

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &id])
        .assert()
        .success();

    assert!(hsearch()[0]["verified_at"].is_string());
}

#[test]
fn given_verify_when_run_then_no_embedding_model_is_loaded() {
    // Agents stamp at session end: this has to stay a plain write, not half a second of model load.
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    let models = dir.path().join("models");
    std::fs::create_dir(&models).unwrap();
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .env("FASTEMBED_CACHE_DIR", &models)
        .args(["verify", &id])
        .assert()
        .success();

    assert_eq!(std::fs::read_dir(&models).unwrap().count(), 0);
}

#[test]
fn given_unverified_bookmark_when_show_then_it_prints_never() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verified: never"));
}

#[test]
fn given_verified_bookmark_when_show_then_it_prints_when() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["verify", &id])
        .assert()
        .success();
    let stamp = verified_at(&db, &id);
    let date = &stamp.as_str().expect("verify sets a stamp")[..10];

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["show", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Verified: {date}")));
}

#[test]
fn given_bookmark_when_hsearch_json_then_it_keeps_its_own_shape() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("verify.db");
    create_db_with_one_bookmark(&db);

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["hsearch", "Example", "--mode", "exact", "--json", "--np"])
        .output()
        .expect("run bkmr hsearch");

    assert!(output.status.success());
    let hits: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("hsearch --json returns a JSON array");
    let hit = hits[0].as_object().expect("a hit is a JSON object");
    let mut keys: Vec<&str> = hit.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "created_at",
            "description",
            "id",
            "rrf_score",
            "tags",
            "title",
            "url",
            "verified_at"
        ]
    );
    assert!(hit["id"].is_i64());
    assert!(hit["rrf_score"].is_f64());
    assert_eq!(hit["tags"], ",show-test,");
}

/// Config file that stops memories being embedded, so these tests need no embedding model.
fn config_without_memory_embedding(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("config.toml");
    std::fs::write(&path, "[embeddings.policy]\n_mem_ = \"off\"\n").unwrap();
    path
}

fn create_db_with_one_memory(db: &std::path::Path, config: &std::path::Path) {
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(db)
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .arg("--config")
        .arg(config)
        .args([
            "add",
            "https://example.com",
            "show-test",
            "--title",
            "Example",
            "-t",
            "mem",
            "--no-web",
        ])
        .assert()
        .success();
}

fn history_of(db: &std::path::Path, args: &[&str]) -> Vec<serde_json::Value> {
    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .arg("history")
        .args(args)
        .arg("--json")
        .output()
        .expect("run bkmr history");
    assert!(
        output.status.success(),
        "bkmr history failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("history --json returns a JSON array")
}

fn run_with_config(
    db: &std::path::Path,
    config: &std::path::Path,
    args: &[&str],
    stdin: &str,
) -> std::process::Output {
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .arg("--config")
        .arg(config)
        .args(args)
        .write_stdin(stdin)
        .output()
        .expect("run bkmr")
}

#[test]
fn given_memory_when_url_changes_then_history_keeps_the_old_text() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    let output = run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );
    assert!(output.status.success());

    let history = history_of(&db, &[&id]);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["change"], "update");
    assert_eq!(history[0]["url"], "https://example.com");
    assert_eq!(history[0]["title"], "Example");
    assert_eq!(history[0]["bookmark_id"].to_string(), id);
}

#[test]
fn given_other_type_when_url_changes_then_no_history_is_kept() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    // Only agent memories keep history.
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["update", "--url", "https://example.com/new", &id])
        .assert()
        .success();

    assert!(history_of(&db, &[&id]).is_empty());
}

#[test]
fn given_memory_when_only_accessed_then_no_history_is_kept() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    let output = run_with_config(&db, &config, &["show", &id], "");
    assert!(output.status.success());

    assert!(history_of(&db, &[&id]).is_empty());
}

#[test]
fn given_update_entry_when_restored_then_old_text_returns_under_the_same_id_and_undo_is_possible() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );
    let history_id = history_of(&db, &[&id])[0]["history_id"].to_string();

    let output = run_with_config(&db, &config, &["restore", &history_id], "");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bookmarks = search_all(&db);
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0]["id"].to_string(), id);
    assert_eq!(bookmarks[0]["url"], "https://example.com");
    // Restoring is a change like any other, so the corrected text can be brought back too.
    assert_eq!(history_of(&db, &[&id])[0]["url"], "https://example.com/new");
}

#[test]
fn given_deleted_memory_when_restored_then_it_is_back_with_its_tags() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["delete", &id], "y\n");
    assert!(search_all(&db).is_empty());

    // Without an ID, history lists everything, which is how a deleted ID is found again.
    let history = history_of(&db, &[]);
    assert_eq!(history[0]["change"], "delete");
    let history_id = history[0]["history_id"].to_string();

    let output = run_with_config(&db, &config, &["restore", &history_id], "");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bookmarks = search_all(&db);
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0]["url"], "https://example.com");
    let tags = bookmarks[0]["tags"].to_string();
    assert!(tags.contains("show-test"), "{tags}");
    assert!(tags.contains("_mem_"), "{tags}");
}

#[test]
fn given_deleted_highest_id_when_adding_then_the_id_is_not_reused_and_restore_returns_under_the_old_id(
) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/v2", &id],
        "",
    );
    run_with_config(&db, &config, &["delete", &id], "y\n");

    add_second_bookmark(&db);
    let second_id = search_all(&db)[0]["id"].to_string();
    assert_ne!(second_id, id, "a deleted ID must not be given out again");

    let oldest = history_of(&db, &[]).last().unwrap().clone();
    assert_eq!(oldest["change"], "update");
    let output = run_with_config(
        &db,
        &config,
        &["restore", &oldest["history_id"].to_string()],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bookmarks = search_all(&db);
    assert_eq!(bookmarks.len(), 2);
    let restored = bookmarks
        .iter()
        .find(|b| b["id"].to_string() == id)
        .expect("restored under its old ID");
    assert_eq!(restored["url"], "https://example.com");
    let other = bookmarks
        .iter()
        .find(|b| b["id"].to_string() == second_id)
        .expect("the other bookmark still exists");
    assert_eq!(other["url"], "https://example.org");
}

#[test]
fn given_memory_that_lost_its_tag_before_delete_when_restoring_then_a_later_bookmark_is_untouched()
{
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/v2", &id],
        "",
    );
    run_with_config(&db, &config, &["update", "-n", "_mem_", &id], "");
    // No longer a memory, so this delete leaves no 'delete' entry behind.
    run_with_config(&db, &config, &["delete", &id], "y\n");
    add_second_bookmark(&db);

    let oldest = history_of(&db, &[]).last().unwrap().clone();
    let output = run_with_config(
        &db,
        &config,
        &["restore", &oldest["history_id"].to_string()],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let urls: Vec<String> = search_all(&db)
        .iter()
        .map(|b| b["url"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(urls.len(), 2, "got {urls:?}");
    assert!(urls.contains(&"https://example.org".to_string()));
    assert!(urls.contains(&"https://example.com".to_string()));
}

#[test]
fn given_memory_deleted_and_restored_when_rolling_back_further_then_no_second_memory_appears() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/v2", &id],
        "",
    );
    run_with_config(&db, &config, &["delete", &id], "y\n");
    let history = history_of(&db, &[]);
    let (delete_entry, oldest) = (history[0].clone(), history[1].clone());
    run_with_config(
        &db,
        &config,
        &["restore", &delete_entry["history_id"].to_string()],
        "",
    );

    let output = run_with_config(
        &db,
        &config,
        &["restore", &oldest["history_id"].to_string()],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bookmarks = search_all(&db);
    assert_eq!(bookmarks.len(), 1);
    assert_eq!(bookmarks[0]["id"].to_string(), id);
    assert_eq!(bookmarks[0]["url"], "https://example.com");
}

#[test]
fn given_row_that_lost_the_memory_tag_when_restoring_then_its_current_text_is_kept_in_history() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["update", "-n", "_mem_", &id], "");
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://plain.example/valuable", &id],
        "",
    );
    let entry = history_of(&db, &[&id]).last().unwrap().clone();

    let output = run_with_config(
        &db,
        &config,
        &["restore", &entry["history_id"].to_string()],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(search_all(&db)[0]["url"], "https://example.com");
    assert_eq!(
        history_of(&db, &[&id])[0]["url"],
        "https://plain.example/valuable"
    );
}

#[test]
fn given_one_update_with_several_flags_when_restoring_the_newest_entry_then_the_command_is_undone()
{
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    let output = run_with_config(
        &db,
        &config,
        &[
            "update",
            &id,
            "-t",
            "newtag",
            "-n",
            "show-test",
            "--url",
            "https://example.com/new",
            "--title",
            "New title",
        ],
        "",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let history = history_of(&db, &[&id]);
    assert_eq!(history.len(), 1, "one command must write one history entry");
    run_with_config(
        &db,
        &config,
        &["restore", &history[0]["history_id"].to_string()],
        "",
    );
    let restored = &search_all(&db)[0];
    assert_eq!(restored["url"], "https://example.com");
    assert_eq!(restored["title"], "Example");
    let tags = restored["tags"].to_string();
    assert!(
        tags.contains("show-test") && !tags.contains("newtag"),
        "{tags}"
    );
}

#[test]
fn given_adding_and_removing_the_same_tag_when_updating_then_no_history_is_written() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    run_with_config(&db, &config, &["update", &id, "-t", "x", "-n", "x"], "");

    assert!(history_of(&db, &[&id]).is_empty());
}

#[test]
fn given_memory_with_opener_and_no_embedding_when_deleted_and_restored_then_both_survive() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[embeddings.policy]\n_mem_ = \"on\"\n").unwrap();
    create_db_with_one_memory_not_embedded(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            "UPDATE bookmarks SET opener = 'less {}', file_path = '/tmp/x.md', \
             file_mtime = 7, file_hash = 'abc'",
        )
        .unwrap();
    run_with_config(&db, &config, &["delete", &id], "y\n");
    let history_id = history_of(&db, &[])[0]["history_id"].to_string();

    let output = run_with_config(&db, &config, &["restore", &history_id], "");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (opener, embeddable, path, mtime, hash): (String, bool, String, i32, String) =
        rusqlite::Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT opener, embeddable, file_path, file_mtime, file_hash FROM bookmarks",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
    assert_eq!(opener, "less {}");
    assert!(!embeddable);
    assert_eq!(path, "/tmp/x.md");
    assert_eq!(mtime, 7);
    assert_eq!(hash, "abc");
}

fn create_db_with_one_memory_not_embedded(db: &std::path::Path, config: &std::path::Path) {
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(db)
        .assert()
        .success();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .arg("--config")
        .arg(config)
        .args([
            "add",
            "https://example.com",
            "show-test",
            "--title",
            "Example",
            "-t",
            "mem",
            "--no-web",
            "--no-embed",
        ])
        .assert()
        .success();
}

#[test]
fn given_memory_imported_from_a_file_when_restoring_in_place_then_refused_and_unchanged() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("UPDATE bookmarks SET file_path = '/tmp/source.md'")
        .unwrap();
    let history_id = history_of(&db, &[&id])[0]["history_id"].to_string();

    let output = run_with_config(&db, &config, &["restore", &history_id], "");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("/tmp/source.md"));
    assert_eq!(search_all(&db)[0]["url"], "https://example.com/new");
}

#[test]
fn given_repeated_id_when_delete_then_deletes_once_and_exits_success() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &format!("{id},{id}")])
        .write_stdin("y\ny\n")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(search_all(&db).is_empty());
}

#[test]
fn given_invalid_id_among_ids_when_delete_then_still_offers_the_rest_and_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("delete.db");
    create_db_with_one_bookmark(&db);
    let id = search_all(&db)[0]["id"].to_string();

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &format!("0,{id}")])
        .write_stdin("y\n")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(search_all(&db).is_empty(), "the valid ID was not deleted");
}

#[test]
fn given_limit_below_one_when_history_then_exits_with_usage_error() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    create_db_with_one_bookmark(&db);

    for limit in ["--limit=0", "--limit=-1"] {
        let output = cargo_bin_cmd!("bkmr")
            .env("BKMR_DB_URL", &db)
            .args(["history", limit])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(64), "{limit}");
    }
}

#[test]
fn given_invalid_id_when_history_then_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    create_db_with_one_bookmark(&db);

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["history", "0"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn given_history_when_listing_as_text_then_the_change_time_is_labelled_utc() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );

    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["history", &id])
        .assert()
        .success()
        .stdout(predicate::str::contains(" UTC"));
}

/// True if the text occurs in the database file or its write-ahead log.
fn database_files_contain(db: &std::path::Path, text: &str) -> bool {
    let wal = db.with_extension("db-wal");
    [db, wal.as_path()].iter().any(|path| {
        std::fs::read(path)
            .map(|bytes| bytes.windows(text.len()).any(|w| w == text.as_bytes()))
            .unwrap_or(false)
    })
}

fn add_env_secret(db: &std::path::Path, secret: &str) {
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", db)
        .args([
            "add",
            &format!("export TOKEN={secret}"),
            "--title",
            "Token",
            "-t",
            "env",
            "--no-web",
            "--no-embed",
        ])
        .assert()
        .success();
}

#[test]
fn given_plain_deleted_env_secret_when_force_delete_then_it_is_gone_from_the_database_files() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    cargo_bin_cmd!("bkmr")
        .arg("create-db")
        .arg(&db)
        .assert()
        .success();
    add_env_secret(&db, "qrlfqmrhlgggddx");
    let id = search_all(&db)[0]["id"].to_string();
    cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", &id])
        .write_stdin("y\n")
        .assert()
        .success();
    assert!(database_files_contain(&db, "qrlfqmrhlgggddx"));

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", "--force", &id])
        .output()
        .unwrap();

    // The ID is still unknown, but the traces of what it held are removed.
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("not found"));
    assert!(!database_files_contain(&db, "qrlfqmrhlgggddx"));
}

#[test]
fn given_secret_when_force_delete_prompts_then_the_prompt_does_not_print_it() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "token qrlfqmrhlgggddx", &id],
        "",
    );

    let output = run_with_config(&db, &config, &["delete", "--force", &id], "n\n");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Example"), "{stderr}");
    assert!(!stderr.contains("qrlfqmrhlgggddx"), "{stderr}");
}

#[test]
fn given_a_purge_then_a_delete_when_searching_the_deleted_id_then_search_succeeds() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let purged = search_all(&db)[0]["id"].to_string();
    for url in ["https://example.com/a", "https://example.com/b"] {
        let output = run_with_config(
            &db,
            &config,
            &[
                "add",
                url,
                "plain",
                "--title",
                "Plain",
                "--no-web",
                "--no-embed",
            ],
            "",
        );
        assert!(output.status.success());
    }
    let deleted = search_all(&db)
        .iter()
        .map(|bookmark| bookmark["id"].as_i64().unwrap())
        .max()
        .unwrap()
        .to_string();

    let output = run_with_config(&db, &config, &["delete", "--force", &purged], "y\n");
    assert!(output.status.success());
    let output = run_with_config(&db, &config, &["delete", &deleted], "y\n");
    assert!(output.status.success());

    let output = run_with_config(&db, &config, &["search", "--np", "--json", &deleted], "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(found.is_empty());
}

#[test]
fn given_other_process_reading_when_force_delete_then_reports_failure_and_a_retry_finishes() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "token qrlfqmrhlgggddx", &id],
        "",
    );
    let reader = rusqlite::Connection::open(&db).unwrap();
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM bookmarks;")
        .ok();
    let _: i64 = reader
        .query_row("SELECT count(*) FROM bookmarks", [], |r| r.get(0))
        .unwrap();

    let output = run_with_config(&db, &config, &["delete", "--force", &id], "y\n");

    assert_eq!(
        output.status.code(),
        Some(1),
        "must not claim a purge while the text can still be on disk"
    );
    assert!(search_all(&db).is_empty());

    reader.execute_batch("COMMIT").unwrap();
    drop(reader);
    let retry = run_with_config(&db, &config, &["delete", "--force", &id], "");
    assert_eq!(retry.status.code(), Some(1), "the ID is unknown by now");
    assert!(!database_files_contain(&db, "qrlfqmrhlgggddx"));
}

#[test]
fn given_unknown_history_id_when_restoring_then_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    create_db_with_one_bookmark(&db);

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["restore", "99999"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_force_when_delete_then_bookmark_and_its_history_are_gone() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );

    let output = run_with_config(&db, &config, &["delete", "--force", &id], "y\n");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(search_all(&db).is_empty());
    assert!(history_of(&db, &[]).is_empty());
}

#[test]
fn given_force_when_bookmark_was_already_deleted_then_its_history_is_purged() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["delete", &id], "y\n");
    assert_eq!(history_of(&db, &[]).len(), 1);

    let output = run_with_config(&db, &config, &["delete", "--force", &id], "y\n");

    assert_eq!(output.status.code(), Some(0));
    assert!(history_of(&db, &[]).is_empty());
}

#[test]
fn given_force_and_declined_prompt_when_delete_then_nothing_is_purged_and_exit_is_cancel() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );

    let output = run_with_config(&db, &config, &["delete", "--force", &id], "n\n");

    // A script running `delete --force` && ... must not read this as the secret being gone.
    assert_eq!(output.status.code(), Some(130));
    assert_eq!(search_all(&db).len(), 1);
    assert_eq!(history_of(&db, &[&id]).len(), 1);
}

#[test]
fn given_force_and_unknown_id_when_delete_then_exits_failure() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    create_db_with_one_bookmark(&db);

    let output = cargo_bin_cmd!("bkmr")
        .env("BKMR_DB_URL", &db)
        .args(["delete", "--force", "99999"])
        .write_stdin("y\n")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(search_all(&db).len(), 1);
}

#[test]
fn given_memory_changed_twice_when_history_with_patch_then_each_step_is_shown_as_a_diff() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/v2", &id],
        "",
    );
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/v3", &id],
        "",
    );

    let output = run_with_config(&db, &config, &["--no-color", "history", &id, "-p"], "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Oldest step: the first text was replaced by v2; newest step: v2 by the current v3.
    assert!(
        stdout.contains("\n-https://example.com\n+https://example.com/v2\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\n-https://example.com/v2\n+https://example.com/v3\n"),
        "{stdout}"
    );
    assert!(stdout.contains(&format!("+++ #{id} current\n")), "{stdout}");
    // Piped stdout is never colored, so this holds with or without --no-color: it guards the
    // diff against escape codes in pipes. The flag's own effect is tested in util::helper.
    assert!(!stdout.contains('\x1b'), "{stdout:?}");
}

#[test]
fn given_deleted_memory_when_history_with_patch_then_the_last_state_is_shown_removed() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["delete", "--yes", &id], "");

    let output = run_with_config(&db, &config, &["--no-color", "history", &id, "-p"], "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("+++ /dev/null\n"), "{stdout}");
    assert!(stdout.contains("\n-https://example.com\n"), "{stdout}");
}

#[test]
fn given_patch_without_memory_id_when_history_then_usage_error() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);

    let output = run_with_config(&db, &config, &["history", "-p"], "");

    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn given_patch_with_json_when_history_then_usage_error() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();

    let output = run_with_config(&db, &config, &["history", &id, "-p", "--json"], "");

    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn given_updated_memory_when_restore_dry_run_then_diff_from_current_to_saved_state_and_nothing_changes(
) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(
        &db,
        &config,
        &["update", "--url", "https://example.com/new", &id],
        "",
    );
    let history_id = history_of(&db, &[&id])[0]["history_id"].to_string();

    let output = run_with_config(
        &db,
        &config,
        &["--no-color", "restore", &history_id, "--dry-run"],
        "",
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("--- #{id} current\n")), "{stdout}");
    assert!(
        stdout.contains("\n-https://example.com/new\n+https://example.com\n"),
        "{stdout}"
    );
    assert_eq!(search_all(&db)[0]["url"], "https://example.com/new");
    assert_eq!(
        history_of(&db, &[&id]).len(),
        1,
        "a dry run must not write history"
    );
}

#[test]
fn given_deleted_memory_when_restore_dry_run_then_every_line_is_added_and_it_stays_deleted() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    run_with_config(&db, &config, &["delete", "--yes", &id], "");
    let history_id = history_of(&db, &[])[0]["history_id"].to_string();

    let output = run_with_config(
        &db,
        &config,
        &["--no-color", "restore", &history_id, "--dry-run"],
        "",
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("--- /dev/null\n"), "{stdout}");
    assert!(stdout.contains("\n+https://example.com\n"), "{stdout}");
    assert!(search_all(&db).is_empty());
}

#[test]
fn given_unknown_history_id_when_restore_dry_run_then_it_fails_like_restore() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);

    let output = run_with_config(&db, &config, &["restore", "99999", "--dry-run"], "");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("History entry 99999 not found"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn given_memory_deleted_without_history_after_last_update_when_history_with_patch_then_update_is_not_shown_as_deletion(
) {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let config = config_without_memory_embedding(dir.path());
    create_db_with_one_memory(&db, &config);
    let id = search_all(&db)[0]["id"].to_string();
    // Recorded, because the row was a memory before the change
    run_with_config(&db, &config, &["update", "-n", "_mem_", &id], "");
    // Not recorded, because it is no longer a memory
    run_with_config(&db, &config, &["delete", "--yes", &id], "");
    assert_eq!(history_of(&db, &[&id]).len(), 1);

    let output = run_with_config(&db, &config, &["--no-color", "history", &id, "-p"], "");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("/dev/null"), "{stdout}");
    assert!(!stdout.contains("\n-https://example.com\n"), "{stdout}");
    assert!(
        stdout.contains(&format!("#{id} no longer exists")),
        "{stdout}"
    );
}
