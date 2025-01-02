#![allow(unused_imports, unused_variables)]

use assert_cmd::prelude::*;
use assert_cmd::Command;
use predicates::prelude::*;
use rstest::*;
use std::fs;

#[rstest]
fn test_debug_mode() {
    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d"]).assert().success();
    // cmd.args(&["-d", "-d"])
    //     .assert()
    //     .stderr(predicate::str::contains("Debug mode: debug"));
}

#[rstest]
fn test_create_db() {
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "create-db", "/tmp/bkmr_test.db"])
        .assert()
        .stdout(predicate::str::contains("Database created"));
}

#[rstest]
fn test_show_bms() {
    fs::remove_file("/tmp/bkmr_test.db").unwrap_or_default();

    let mut cmd = Command::cargo_bin("bkmr").unwrap();
    cmd.args(["-d", "-d", "show", "1,2"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Debug mode: debug"))
        .stderr(predicate::str::contains("Google"));
}
