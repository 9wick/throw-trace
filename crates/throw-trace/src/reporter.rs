use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use throw_trace_core::{Diagnostic, LspViolation, PropagatedThrow};

// throw 元ファイルを読んでバイトオフセットを 1-based 行番号へ変換する。
// 同一ファイルへの重複 I/O を避けるため report 呼び出し単位でキャッシュする
#[derive(Default)]
struct OriginResolver {
    sources: HashMap<PathBuf, Option<String>>,
}

impl OriginResolver {
    fn line_of(&mut self, file_path: &Path, byte_offset: u32) -> Option<u32> {
        let source = self
            .sources
            .entry(file_path.to_path_buf())
            .or_insert_with(|| std::fs::read_to_string(file_path).ok())
            .as_ref()?;
        let offset = byte_offset as usize;
        if offset > source.len() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        Some(source[..offset].matches('\n').count() as u32 + 1)
    }

    fn format_origin(&mut self, throw: &PropagatedThrow) -> String {
        let path = &throw.origin_function.file_path;
        match self.line_of(path, throw.origin.location.start) {
            Some(line) => format!("{}:{line}", path.display()),
            None => path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Serialize)]
struct JsonReport {
    diagnostics: Vec<JsonDiagnostic>,
    lsp_violations: Vec<JsonLspViolation>,
    summary: Summary,
}

#[derive(Serialize)]
struct JsonDiagnostic {
    file: String,
    function: String,
    missing_throws: Vec<JsonMissingThrow>,
}

#[derive(Serialize)]
struct JsonMissingThrow {
    error_type: String,
    origin_file: String,
    // throw 元ファイルが読めない場合は偽の数値を出さず null にする
    origin_line: Option<u32>,
}

#[derive(Serialize)]
struct JsonLspViolation {
    file: String,
    class: String,
    method: String,
    illegal_throws: Vec<String>,
    parent_type: String,
    parent_declared_throws: Vec<String>,
}

#[derive(Serialize)]
struct Summary {
    errors: usize,
    lsp_violations: usize,
    files_checked: usize,
}

pub fn report(
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
    format: OutputFormat,
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    report_to(&mut stdout, diagnostics, lsp_violations, files_checked, format)
}

fn report_to<W: Write>(
    writer: &mut W,
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Text => report_text(writer, diagnostics, lsp_violations, files_checked),
        OutputFormat::Json => report_json(writer, diagnostics, lsp_violations, files_checked),
    }
}

fn report_text<W: Write>(
    stdout: &mut W,
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
) -> io::Result<()> {
    if diagnostics.is_empty() && lsp_violations.is_empty() {
        writeln!(stdout, "No issues found in {files_checked} files")?;
        return Ok(());
    }

    let mut resolver = OriginResolver::default();

    for diag in diagnostics {
        writeln!(stdout, "error: missing @throws declaration")?;
        writeln!(stdout, "  --> {}:{}", diag.function.file_path.display(), diag.function.name)?;
        writeln!(stdout, "   |")?;

        for missing in &diag.missing_throws {
            let type_name = missing.error_type.type_name().unwrap_or("Unknown");
            let origin = resolver.format_origin(missing);
            writeln!(stdout, "   | {type_name} propagates from {origin}")?;
        }

        writeln!(stdout, "   |")?;
        writeln!(
            stdout,
            "   = help: add @throws {{{}}} to function {}",
            help_types(&diag.missing_throws),
            diag.function.name
        )?;
        writeln!(stdout)?;
    }

    for violation in lsp_violations {
        writeln!(stdout, "error: LSP violation - throws not declared in parent")?;
        writeln!(
            stdout,
            "  --> {}:{}",
            violation.implementation.file_path.display(),
            violation.implementation.name
        )?;
        writeln!(stdout, "   |")?;

        for illegal in &violation.illegal_throws {
            let type_name = illegal.type_name().unwrap_or("Unknown");
            writeln!(
                stdout,
                "   | {} is not declared in {}.{}",
                type_name,
                violation.parent_method.type_id.name,
                violation.parent_method.method_name
            )?;
        }

        writeln!(stdout, "   |")?;
        let parent_throws: Vec<_> =
            violation.parent_method.declared_throws.iter().map(|d| d.error_type.as_str()).collect();
        if parent_throws.is_empty() {
            writeln!(stdout, "   = parent declares: (no throws allowed)")?;
        } else {
            writeln!(stdout, "   = parent declares: @throws {{{}}}", parent_throws.join(", "))?;
        }
        writeln!(
            stdout,
            "   = help: handle the exception in the implementation or add @throws to the parent"
        )?;
        writeln!(stdout)?;
    }

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();
    let violation_count = lsp_violations.len();
    writeln!(
        stdout,
        "Found {error_count} errors, {violation_count} LSP violations in {files_checked} files"
    )?;

    Ok(())
}

// 型解決できなかった throw は `unknown` として提示する。空の型リスト
// `@throws {}` は構文として成立せず、利用者が修正手段に辿り着けないため
fn help_types(missing: &[throw_trace_core::PropagatedThrow]) -> String {
    let mut types: Vec<&str> =
        missing.iter().map(|m| m.error_type.type_name().unwrap_or("unknown")).collect();
    types.dedup();
    types.join(", ")
}

fn report_json<W: Write>(
    writer: &mut W,
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
) -> io::Result<()> {
    let mut resolver = OriginResolver::default();
    let json_diagnostics: Vec<JsonDiagnostic> = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            file: d.function.file_path.display().to_string(),
            function: d.function.name.to_string(),
            missing_throws: d
                .missing_throws
                .iter()
                .map(|m| JsonMissingThrow {
                    error_type: m.error_type.type_name().unwrap_or("Unknown").to_string(),
                    origin_file: m.origin_function.file_path.display().to_string(),
                    origin_line: resolver
                        .line_of(&m.origin_function.file_path, m.origin.location.start),
                })
                .collect(),
        })
        .collect();

    let json_lsp_violations: Vec<JsonLspViolation> = lsp_violations
        .iter()
        .map(|v| JsonLspViolation {
            file: v.implementation.file_path.display().to_string(),
            class: String::new(),
            method: v.implementation.name.to_string(),
            illegal_throws: v
                .illegal_throws
                .iter()
                .map(|e| e.type_name().unwrap_or("Unknown").to_string())
                .collect(),
            parent_type: v.parent_method.type_id.name.to_string(),
            parent_declared_throws: v
                .parent_method
                .declared_throws
                .iter()
                .map(|d| d.error_type.to_string())
                .collect(),
        })
        .collect();

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();
    let violation_count = lsp_violations.len();

    let report = JsonReport {
        diagnostics: json_diagnostics,
        lsp_violations: json_lsp_violations,
        summary: Summary { errors: error_count, lsp_violations: violation_count, files_checked },
    };

    let json = serde_json::to_string_pretty(&report)?;
    writeln!(writer, "{json}")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use throw_trace_core::{ErrorType, FunctionId, PropagatedThrow, Span, ThrowSite};

    fn propagated(error_type: ErrorType) -> PropagatedThrow {
        let span = Span { start: 0, end: 10 };
        PropagatedThrow {
            error_type: error_type.clone(),
            origin: ThrowSite { location: span, error_type },
            origin_function: FunctionId::new(PathBuf::from("a.ts"), "origin", span),
            path: vec![],
        }
    }

    // Unknown のままの throw も help が実行可能な宣言（@throws {unknown}）を提示すること。
    // 空の型リスト `@throws {}` は構文として成立せず、利用者が修正手段に辿り着けない
    #[test]
    fn help_types_renders_unknown_as_declarable_type() {
        let missing = vec![propagated(ErrorType::Unknown)];
        assert_eq!(help_types(&missing), "unknown");
    }

    #[test]
    fn help_types_joins_named_types() {
        let missing =
            vec![propagated(ErrorType::Named("AppError".into())), propagated(ErrorType::Unknown)];
        assert_eq!(help_types(&missing), "AppError, unknown");
    }

    fn diagnostic_with_origin(origin_path: PathBuf, throw_offset: u32) -> Diagnostic {
        let caller =
            FunctionId::new(PathBuf::from("caller.ts"), "callA", Span { start: 0, end: 50 });
        let origin_fn = FunctionId::new(origin_path, "a", Span { start: 0, end: 40 });
        Diagnostic {
            function: caller,
            missing_throws: vec![PropagatedThrow {
                error_type: ErrorType::Named("E1".into()),
                origin: ThrowSite {
                    location: Span { start: throw_offset, end: throw_offset + 14 },
                    error_type: ErrorType::Named("E1".into()),
                },
                origin_function: origin_fn,
                path: vec![],
            }],
        }
    }

    #[test]
    fn json_report_outputs_origin_file_and_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let origin_path = dir.path().join("origin.ts");
        // "throw" は2行目（バイトオフセット17）
        std::fs::write(&origin_path, "function a() {\n  throw new E1();\n}\n").unwrap();

        let diag = diagnostic_with_origin(origin_path.clone(), 17);
        let mut buf = Vec::new();
        report_to(&mut buf, &[diag], &[], 1, OutputFormat::Json).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        let missing = &parsed["diagnostics"][0]["missing_throws"][0];
        assert_eq!(
            missing["origin_file"].as_str().unwrap(),
            origin_path.display().to_string(),
            "origin_file must contain the origin function's file path"
        );
        assert_eq!(
            missing["origin_line"].as_u64().unwrap(),
            2,
            "origin_line must be a 1-based line number, not a byte offset"
        );
    }

    #[test]
    fn json_report_origin_line_is_null_when_file_unreadable() {
        let diag = diagnostic_with_origin(PathBuf::from("/nonexistent/origin.ts"), 17);
        let mut buf = Vec::new();
        report_to(&mut buf, &[diag], &[], 1, OutputFormat::Json).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        let missing = &parsed["diagnostics"][0]["missing_throws"][0];
        assert!(
            missing["origin_line"].is_null(),
            "unknown line must be null, not a fake number: {missing}"
        );
    }

    #[test]
    fn text_report_shows_origin_location_not_span_debug() {
        let dir = tempfile::tempdir().unwrap();
        let origin_path = dir.path().join("origin.ts");
        std::fs::write(&origin_path, "function a() {\n  throw new E1();\n}\n").unwrap();

        let diag = diagnostic_with_origin(origin_path.clone(), 17);
        let mut buf = Vec::new();
        report_to(&mut buf, &[diag], &[], 1, OutputFormat::Text).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains("Span {"), "internal Span debug must not leak: {output}");
        let expected = format!("{}:2", origin_path.display());
        assert!(
            output.contains(&expected),
            "text output must show origin as file:line ({expected}): {output}"
        );
    }
}
