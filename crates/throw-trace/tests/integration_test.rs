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


#[test]
#[ignore = "requires tsserver installed"]
fn check_type_alias_with_resolver_passes() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/type_alias.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_declared_type_alias_resolves() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args([
            "check",
            "tests/fixtures/type_alias_declared.ts",
            "tests/fixtures/errors.ts",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_inheritance_derived_to_base_passes() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/inheritance.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("BaseError"))
        .stdout(predicate::str::contains("catchesDerived"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_static_factory_method_resolves() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/static_factory.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_result_error_property_resolves() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/result_error.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_cross_file_missing_throws_detected() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/cross_file/b.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"))
        .stdout(predicate::str::contains("useValidate"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_cross_file_with_declaration_passes() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/cross_file/a.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
#[ignore = "requires tsserver installed"]
fn check_cross_file_circular_no_infinite_loop() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/cross_file_circular/a.ts"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure()
        .stdout(predicate::str::contains("ErrorB propagates"));
}
