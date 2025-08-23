use assert_cmd::Command;
use bkmr::util::testing::EnvGuard;
use predicates::prelude::*;
use std::fs;

#[test]
fn given_debug_flag_when_running_then_enables_debug_mode() {
    // let config = init_test_env();
    // let _guard = EnvGuard::new();
    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d"]).assert().success();
    // cmd.args(&["-d", "-d"])
    //     .assert()
    //     .stderr(predicate::str::contains("Debug mode: debug"));
}

#[test]
#[ignore = "not implemented"]
fn given_path_when_creating_database_then_creates_successfully() {
    // let config = init_test_env();
    // let _guard = EnvGuard::new();
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "create-db", "/tmp/bkmr_test.db"])
        .assert()
        .stdout(predicate::str::contains("Database created"));
}

#[test]
fn given_bookmark_ids_when_showing_then_displays_correct_entries() {
    // let config = init_test_env();
    let _guard = EnvGuard::new();
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "show", "1,2"]).assert().success();
}
