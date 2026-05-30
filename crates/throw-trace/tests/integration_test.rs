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
        .args(["check", "tests/fixtures/type_alias_declared.ts", "tests/fixtures/errors.ts"])
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

// =============================================================
// メンバ呼び出し（obj.method()）の throws 伝播
// =============================================================

#[test]
fn member_call_propagates_throws_to_caller() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/member_call_propagation.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"))
        .stdout(predicate::str::contains("createUser"));
}

// =============================================================
// catch-all（型ガードなし catch）による例外の全捕捉
// =============================================================

#[test]
fn catch_all_suppresses_all_throws() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/catch_all_suppression.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

// =============================================================
// instanceof 捕捉 + rethrow 時の分岐終端判定
// =============================================================

// if-body が return で終端 → instanceof マッチ型は捕捉済み
#[test]
fn instanceof_with_return_catches_matched_type() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/instanceof_catch_with_rethrow.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

// if-body が終端しない（fall-through）→ rethrow で投げ直される → 未捕捉
#[test]
fn instanceof_fallthrough_does_not_catch() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.current_dir(workspace_root())
        .args(["check", "tests/fixtures/instanceof_fallthrough_rethrow.ts"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"))
        .stdout(predicate::str::contains("logAndRethrow"));
}

// =============================================================
// fix テスト
// =============================================================

#[test]
fn fix_inserts_throws_declaration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.ts");

    std::fs::write(
        &test_file,
        r#"function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {}
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.arg("fix")
        .arg(test_file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fixed 1 file"));

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("@throws {ValidationError}"));
}

#[test]
fn fix_does_not_modify_documented_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.ts");

    let original = r#"/**
 * @throws {ValidationError} When input is invalid
 */
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {}
"#;

    std::fs::write(&test_file, original).unwrap();

    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.arg("fix")
        .arg(test_file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fixed 0 file"));

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, original);
}

#[test]
fn fix_nonexistent_path() {
    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.args(["fix", "nonexistent/path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No TypeScript files found"));
}

#[test]
fn fix_appends_to_existing_jsdoc() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.ts");

    std::fs::write(
        &test_file,
        r#"/**
 * Validates user input.
 * @param input - The input to validate
 */
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {}
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.arg("fix")
        .arg(test_file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fixed 1 file"));

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("@throws {ValidationError}"));
    assert!(content.contains("@param input"));
    assert!(content.contains("Validates user input"));
}

#[test]
fn fix_appends_missing_throws_to_jsdoc_with_only_description() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.ts");

    std::fs::write(
        &test_file,
        r#"/**
 * Validates user input
 */
function validate(input: string) {
  if (!input) {
    throw new ValidationError("Input required");
  }
}

class ValidationError extends Error {}
"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.arg("fix")
        .arg(test_file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fixed 1 file"));

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("@throws {ValidationError}"));
    assert!(content.contains("Validates user input"));
}

#[test]
fn fix_handles_crlf_line_endings() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.ts");

    let crlf_content = "function validate(input: string) {\r\n  if (!input) {\r\n    throw new ValidationError(\"Input required\");\r\n  }\r\n}\r\n\r\nclass ValidationError extends Error {}\r\n";
    std::fs::write(&test_file, crlf_content).unwrap();

    let mut cmd = Command::cargo_bin("throw-trace").unwrap();
    cmd.arg("fix")
        .arg(test_file.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Fixed 1 file"));

    let content = std::fs::read_to_string(&test_file).unwrap();
    assert!(content.contains("@throws {ValidationError}"));
    assert!(content.contains("function validate"));
}
