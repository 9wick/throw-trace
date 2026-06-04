use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;

#[test]
fn check_creates_persistent_cache_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    for _ in 0..2 {
        Command::cargo_bin("throw-trace")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", source.to_str().unwrap()])
            .assert()
            .failure()
            .stdout(predicate::str::contains("missing @throws"));
    }

    let cache = fs::read_to_string(temp.path().join(".throw-trace/cache.json")).unwrap();
    let cache: Value = serde_json::from_str(&cache).unwrap();
    let files = cache["files"].as_object().unwrap();
    assert!(!files.is_empty());

    let entry = files.values().next().unwrap();
    assert!(entry["content_hash"].is_string());
    assert!(!entry["extraction"]["signatures"].as_array().unwrap().is_empty());
}

#[test]
fn changing_source_updates_cache_file() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure();

    let cache_path = temp.path().join(".throw-trace/cache.json");
    let first = fs::read_to_string(&cache_path).unwrap();

    fs::write(
        &source,
        r#"
/**
 * @throws {Error} documented
 */
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No issues found"));

    let second = fs::read_to_string(&cache_path).unwrap();
    assert_ne!(first, second);
}

#[test]
fn schema_mismatch_cache_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));

    let cache_path = temp.path().join(".throw-trace/cache.json");
    let mut cache: Value = serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
    cache["schema_version"] = Value::from(999);
    for entry in cache["files"].as_object_mut().unwrap().values_mut() {
        entry["extraction"]["signatures"] = Value::Array(Vec::new());
        entry["extraction"]["method_signatures"] = Value::Array(Vec::new());
        entry["extraction"]["type_relations"] = Value::Array(Vec::new());
    }
    fs::write(&cache_path, serde_json::to_string(&cache).unwrap()).unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}

#[test]
fn corrupt_cache_json_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join(".throw-trace");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("cache.json"), "{not json").unwrap();

    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", source.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing @throws"));
}

#[test]
fn fix_uses_shared_analyzer_cache_path() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("sample.ts");
    fs::write(
        &source,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["fix", source.to_str().unwrap()])
        .assert()
        .success();

    assert!(temp.path().join(".throw-trace/cache.json").exists());
    let fixed = fs::read_to_string(&source).unwrap();
    assert!(fixed.contains("@throws {Error}"));
}
