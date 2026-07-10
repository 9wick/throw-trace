use assert_cmd::Command;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

#[test]
fn legacy_catalog_has_no_missing_throws() {
    let catalog_dir = workspace_root().join("tests/catalog");
    let entries: Vec<_> = std::fs::read_dir(&catalog_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", catalog_dir.display()))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();

    assert!(!entries.is_empty(), "no .ts files found in tests/catalog/");

    let mut failures = Vec::new();

    for entry in &entries {
        let src = entry.path();
        let file_name = src.file_name().unwrap();
        let assertion = Command::cargo_bin("throw-trace")
            .unwrap()
            .current_dir(workspace_root())
            .args(["check", src.to_str().unwrap()])
            .assert()
            .try_success();

        if let Err(error) = assertion {
            let name = file_name.to_string_lossy();
            failures.push(format!("--- {name} ---\n{error}"));
        }
    }

    assert!(
        failures.is_empty(),
        "legacy catalog check failed for {} file(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn sync_catalog_matches_expected_and_is_idempotent() {
    let cases_dir = workspace_root().join("tests/catalog/cases");
    let entries: Vec<_> = std::fs::read_dir(&cases_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", cases_dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .collect();

    assert!(!entries.is_empty(), "no sync cases found in {}", cases_dir.display());

    let temp_dir = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();

    for entry in entries {
        let case_name = entry.file_name();
        let before = entry.path().join("before.ts");
        let expected = entry.path().join("expected.ts");
        let temp_case_dir = temp_dir.path().join(&case_name);
        std::fs::create_dir_all(&temp_case_dir).unwrap();
        let actual = temp_case_dir.join("before.ts");
        std::fs::copy(&before, &actual).unwrap();

        run_fix(&actual);

        let expected_source = std::fs::read_to_string(&expected).unwrap();
        let first_fixed = std::fs::read_to_string(&actual).unwrap();
        if expected_source != first_fixed {
            failures.push(format!(
                "--- {} (expected) ---\n{}",
                case_name.to_string_lossy(),
                diff_strings(&expected_source, &first_fixed)
            ));
            continue;
        }

        run_fix(&actual);
        let second_fixed = std::fs::read_to_string(&actual).unwrap();
        if first_fixed != second_fixed {
            failures.push(format!(
                "--- {} (idempotency) ---\n{}",
                case_name.to_string_lossy(),
                diff_strings(&first_fixed, &second_fixed)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "sync catalog failed for {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn run_fix(path: &Path) {
    Command::cargo_bin("throw-trace")
        .unwrap()
        .current_dir(workspace_root())
        .args(["fix", path.to_str().unwrap()])
        .assert()
        .success();
}

fn diff_strings(original: &str, modified: &str) -> String {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();
    let mut result = String::new();

    let max = orig_lines.len().max(mod_lines.len());
    for i in 0..max {
        let o = orig_lines.get(i).copied().unwrap_or("");
        let m = mod_lines.get(i).copied().unwrap_or("");
        if o != m {
            if !o.is_empty() {
                result.push_str(&format!("-{o}\n"));
            }
            if !m.is_empty() {
                result.push_str(&format!("+{m}\n"));
            }
        }
    }
    result
}
