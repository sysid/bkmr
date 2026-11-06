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
