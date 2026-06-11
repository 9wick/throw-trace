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

// 無関係なファイルの変更で diagnostics キャッシュ全体が無効化されないこと。
// fingerprint がワークスペース全体のソースハッシュに依存していると、
// 1ファイルの変更で全関数のキャッシュが miss する
#[test]
fn unrelated_file_change_keeps_diagnostic_cache_entries() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("a.ts");
    let unrelated = temp.path().join("b.ts");
    fs::write(
        &target,
        r#"
function raises() {
  throw new Error("x");
}
"#,
    )
    .unwrap();
    fs::write(&unrelated, "export const x = 1;\n").unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "."])
        .assert()
        .code(1);

    let cache_path = temp.path().join(".throw-trace/cache.json");
    let first: Value = serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
    let first_diags = first["diagnostics"].as_object().unwrap().clone();
    assert!(!first_diags.is_empty(), "diagnostics cache should be populated");

    // 無関係ファイルだけを変更する（関数は含まれないので diagnostics エントリは増えない）
    fs::write(&unrelated, "export const x = 1;\nexport const y = 2;\n").unwrap();

    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "."])
        .assert()
        .code(1);

    let second: Value = serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
    let second_diags = second["diagnostics"].as_object().unwrap();

    for (key, entry) in &first_diags {
        let second_entry = second_diags
            .get(key)
            .unwrap_or_else(|| panic!("diagnostic entry {key} should survive unrelated change"));
        assert_eq!(
            second_entry["dependency_fingerprint"], entry["dependency_fingerprint"],
            "dependency fingerprint must not change when an unrelated file changes"
        );
    }
}
