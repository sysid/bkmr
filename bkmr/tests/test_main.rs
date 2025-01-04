#![allow(unused_imports, unused_variables)]

use assert_cmd::prelude::*;
use assert_cmd::Command;
use predicates::prelude::*;
use rstest::*;
use std::fs;

#[rstest]
fn given_debug_flag_when_running_then_enables_debug_mode() {
    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d"]).assert().success();
    // cmd.args(&["-d", "-d"])
    //     .assert()
    //     .stderr(predicate::str::contains("Debug mode: debug"));
}

#[rstest]
fn given_path_when_creating_database_then_creates_successfully() {
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "create-db", "/tmp/bkmr_test.db"])
        .assert()
        .stdout(predicate::str::contains("Database created"));
}

#[rstest]
fn given_bookmark_ids_when_showing_then_displays_correct_entries() {
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "show", "1,2"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Debug mode: debug"))
        .stderr(predicate::str::contains("Google"));
}
