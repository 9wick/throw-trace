use assert_cmd::Command;
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap()
}

#[test]
fn catalog_fix_is_idempotent() {
    let catalog_dir = workspace_root().join("tests/catalog");
    let entries: Vec<_> = std::fs::read_dir(&catalog_dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", catalog_dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();

    assert!(!entries.is_empty(), "no .ts files found in tests/catalog/");

    let temp_dir = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();

    for entry in &entries {
        let src = entry.path();
        let file_name = src.file_name().unwrap();
        let tmp = temp_dir.path().join(file_name);
        std::fs::copy(&src, &tmp).unwrap();

        Command::cargo_bin("throw-trace")
            .unwrap()
            .current_dir(workspace_root())
            .args(["fix", tmp.to_str().unwrap()])
            .assert()
            .success();

        let original = std::fs::read_to_string(&src).unwrap();
        let fixed = std::fs::read_to_string(&tmp).unwrap();

        if original != fixed {
            let diff = diff_strings(&original, &fixed);
            failures.push(format!("--- {} ---\n{}", file_name.to_string_lossy(), diff));
        }
    }

    if !failures.is_empty() {
        panic!(
            "catalog idempotency check failed for {} file(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
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
                result.push_str(&format!("-{}\n", o));
            }
            if !m.is_empty() {
                result.push_str(&format!("+{}\n", m));
            }
        }
    }
    result
}
