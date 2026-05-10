// crates/throw-trace/tests/integration_test.rs

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

// CARGO_MANIFEST_DIR is the crate dir (crates/throw-trace).
// The workspace root is two levels up from there.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

#[test]
fn check_simple_throw_reports_missing() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/simple_throw.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}

#[test]
fn check_with_json_format() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/simple_throw.ts", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"diagnostics\""));
}

#[test]
fn check_nonexistent_path() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "nonexistent/path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No TypeScript files found"));
}
