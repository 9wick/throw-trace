use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

#[test]
fn unrelated_type_errors_do_not_invalidate_throws_declarations() {
    for fixture in ["assignability_clean.ts", "assignability_existing_type_error.ts"] {
        Command::cargo_bin("throw-trace")
            .unwrap()
            .current_dir(workspace_root())
            .args(["check", &format!("tests/fixtures/false_positives/{fixture}")])
            .assert()
            .success()
            .stdout(predicate::str::contains("No issues found"));
    }
}

#[test]
fn returned_callbacks_do_not_throw_during_creation() {
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", "tests/fixtures/false_positives/returned_callbacks.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn member_call_does_not_resolve_to_an_unrelated_local_function() {
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", "tests/fixtures/false_positives/unrelated_run.ts"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));
}

#[test]
fn incompatible_types_still_report_a_missing_declaration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    // No final newline, a trailing comment, and a conflicting probe name must
    // not prevent the synthetic assignment from being checked.
    std::fs::write(&file, "const unrelated: string = 123;\nconst _typeCheck = 0;\n/** @throws {string} */\nfunction incompatible() { throw new TypeError(); } // 😀").unwrap();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("incompatible"))
        .stdout(predicate::str::contains("TypeError"));
}

#[test]
fn inline_calls_still_propagate_throws_to_the_caller() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    std::fs::write(
        &file,
        r"
/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }
function callsArrow() { (/** @throws {TypeError} */ () => dangerous())(); }
function callsFunction() { (/** @throws {TypeError} */ function () { dangerous(); })(); }
function callsNamedFunction() { (/** @throws {TypeError} */ function inline() { dangerous(); })(); }
",
    )
    .unwrap();
    let output = Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap(), "--format", "json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let mut functions: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic["function"].as_str().unwrap())
        .collect();
    functions.sort_unstable();
    assert_eq!(functions, ["callsArrow", "callsFunction", "callsNamedFunction"]);
}

#[test]
fn definitions_on_the_same_line_are_distinguished() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    std::fs::write(&file, r"
const marker = '😀'; interface Scope { run(fn: () => void): void } /** @throws {RangeError} */ function run() { throw new RangeError(); }
export function safe(scope: Scope) { scope.run(() => undefined); }
export function unsafe() { run(); }
").unwrap();
    let output = Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap(), "--format", "json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["function"], "unsafe");
}

#[test]
fn object_and_nested_union_throws_are_recognized() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    std::fs::write(&file, r"
/** @throws {TypeError | {} | null} */
export function runBuild(value: {} | null) { throw value; }
/** @throws {{ code: string | number; details: { message: string } }} */
export function structured(value: { code: string | number; details: { message: string } }) { throw value; }
const OtherError = class extends Error {};
/** @throws {OtherError | {} | null} */
export function withValueError(value: {} | null) { throw value; }
").unwrap();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["fix", file.to_str().unwrap()])
        .assert()
        .success();
    let first = std::fs::read_to_string(&file).unwrap();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["fix", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), first);
}

#[test]
fn captured_catch_errors_keep_their_original_types() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    std::fs::write(&file, r"
/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }
/** @throws {Error} */
export function handled() {
  try { dangerous(); } catch (error) { return (() => { throw error; })(); }
}
export function missing() {
  try { dangerous(); } catch (error) { return (() => { throw error; })(); }
}
export function deferred() {
  try { dangerous(); } catch (error) { return () => { throw error; }; }
}
export function caughtInside() {
  try { dangerous(); } catch (error) { return (() => { try { throw error; } catch {} })(); }
}
/** @throws {RangeError} */
export function shadowed() {
  try { dangerous(); } catch (error) { return ((error: RangeError) => { throw error; })(new RangeError()); }
}
/** @throws {RangeError} */
export function nestedCatch() {
  try { try { dangerous(); } catch {} throw new RangeError(); }
  catch (error) { return (() => { throw error; })(); }
}
/** @throws {Error} */
export function nestedCapture() {
  try { dangerous(); } catch (outer) {
    return (() => {
      try { throw outer; } catch (inner) { return (() => { throw inner; })(); }
    })();
  }
}
").unwrap();
    let output = Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap(), "--format", "json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{report}");
    assert_eq!(diagnostics[0]["function"], "missing");
    assert_eq!(diagnostics[0]["missing_throws"][0]["error_type"], "TypeError");
}

#[test]
fn anonymous_callbacks_are_analyzed_without_requiring_their_own_declarations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    std::fs::write(
        &file,
        r"
/** @throws {TypeError} */
function dangerous() { throw new TypeError(); }
export function deferred() { return () => dangerous(); }
export function invokes() { (() => dangerous())(); }
export function iterates() { [1].forEach(() => dangerous()); }
/** @throws {Error} */
export function documented() { (() => dangerous())(); }
export function anonymous_L10() { dangerous(); }
export const bound = function () { dangerous(); };
export function invokesBound() { bound(); }
",
    )
    .unwrap();
    let output = Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap(), "--format", "json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let mut names: Vec<_> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnostic| diagnostic["function"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["anonymous_L10", "bound", "invokes", "invokesBound", "iterates"]);
    // The fixer must document the named callers and leave the anonymous bodies intact.
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["fix", file.to_str().unwrap()])
        .assert()
        .success();
    let first = std::fs::read_to_string(&file).unwrap();
    assert!(first.contains("(() => dangerous())()"));
    assert!(first.contains("return () => dangerous();"));
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["check", file.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["fix", file.to_str().unwrap()])
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(file).unwrap(), first);
}

#[test]
fn cached_captured_errors_change_when_the_original_try_changes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("probe.ts");
    let source = r"
class ErrorA extends Error { readonly kind = 'a'; }
class ErrorB extends Error { readonly kind = 'b'; }
/** @throws {ErrorA | ErrorB} */
function dangerous() { throw new ErrorA(); }
export function factory() {
  try { dangerous(); } catch (error) {
    /** @throws {Error} */
    function captured() { throw error; }
    function caller() { captured(); }
    return caller;
  }
}
";
    for (text, expected) in [
        (source.to_owned(), "ErrorA"),
        (source.replace("throw new ErrorA()", "throw new ErrorB()"), "ErrorB"),
    ] {
        std::fs::write(&file, text).unwrap();
        let output = Command::cargo_bin("throw-trace")
            .unwrap()
            .current_dir(temp_dir.path())
            .args(["check", "probe.ts", "--format", "json"])
            .assert()
            .code(1)
            .get_output()
            .stdout
            .clone();
        let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let diagnostics = report["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1, "{report}");
        assert_eq!(diagnostics[0]["function"], "caller");
        assert_eq!(diagnostics[0]["missing_throws"][0]["error_type"], expected);
    }
}
