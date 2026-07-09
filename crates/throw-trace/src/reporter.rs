use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use throw_trace_core::{Diagnostic, LspViolation, PropagatedThrow};
use throw_trace_ts::byte_offset_to_line_col;

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
    origin_line: Option<u32>,
    origin_function: String,
    path: Vec<JsonPathEntry>,
}

#[derive(Serialize)]
struct JsonPathEntry {
    file: String,
    line: Option<u32>,
    function: String,
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
    match format {
        OutputFormat::Text => report_text(diagnostics, lsp_violations, files_checked),
        OutputFormat::Json => report_json(diagnostics, lsp_violations, files_checked),
    }
}

// 呼び出し元ごとに読み直すと同じファイルを何度も fs アクセスすることになるため、
// パスごとに結果をキャッシュする。読み込み失敗は None として記録し、以後も
// 再試行せず None を返し続ける（存在しないファイルへの毎回のシステムコールを避ける）。
fn fs_source_loader() -> impl FnMut(&Path) -> Option<String> {
    let mut cache: HashMap<PathBuf, Option<String>> = HashMap::new();
    move |path: &Path| {
        cache
            .entry(path.to_path_buf())
            .or_insert_with(|| std::fs::read_to_string(path).ok())
            .clone()
    }
}

// ソースが読めない、またはオフセットがソース長を超える場合は None を返す。
// 呼び出し側はこれを画面表示上のフォールバック（`?`）として扱う。解析済み
// ファイルが実行後に削除されている等の稀なケースであり、握り潰しではない。
fn resolve_line(
    loader: &mut impl FnMut(&Path) -> Option<String>,
    file: &Path,
    offset: u32,
) -> Option<u32> {
    let source = loader(file)?;
    if offset as usize > source.len() {
        return None;
    }
    Some(byte_offset_to_line_col(&source, offset).0)
}

fn line_or_unknown(line: Option<u32>) -> String {
    line.map_or_else(|| "?".to_string(), |l| l.to_string())
}

fn report_text(
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
) -> io::Result<()> {
    let mut loader = fs_source_loader();
    let rendered = render_text(diagnostics, lsp_violations, files_checked, &mut loader);
    let mut stdout = io::stdout().lock();
    write!(stdout, "{rendered}")
}

// レンダリングをソース読み込みから切り離し、固定文字列を返すローダーで
// テストできるようにするための純関数。
fn render_text(
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
    loader: &mut impl FnMut(&Path) -> Option<String>,
) -> String {
    let mut out = String::new();

    if diagnostics.is_empty() && lsp_violations.is_empty() {
        writeln!(out, "No issues found in {files_checked} files").unwrap();
        return out;
    }

    for diag in diagnostics {
        let header_line = line_or_unknown(resolve_line(
            loader,
            &diag.function.file_path,
            diag.function.span.start,
        ));
        writeln!(out, "error: missing @throws declaration").unwrap();
        writeln!(
            out,
            "  --> {}:{} ({})",
            diag.function.file_path.display(),
            header_line,
            diag.function.name
        )
        .unwrap();
        writeln!(out, "   |").unwrap();

        for missing in &diag.missing_throws {
            render_propagation_block(&mut out, missing, loader);
        }

        writeln!(
            out,
            "   = help: add @throws {{{}}} to function {}",
            help_types(&diag.missing_throws),
            diag.function.name
        )
        .unwrap();
        writeln!(out).unwrap();
    }

    for violation in lsp_violations {
        writeln!(out, "error: LSP violation - throws not declared in parent").unwrap();
        writeln!(
            out,
            "  --> {}:{}",
            violation.implementation.file_path.display(),
            violation.implementation.name
        )
        .unwrap();
        writeln!(out, "   |").unwrap();

        for illegal in &violation.illegal_throws {
            let type_name = illegal.type_name().unwrap_or("Unknown");
            writeln!(
                out,
                "   | {} is not declared in {}.{}",
                type_name,
                violation.parent_method.type_id.name,
                violation.parent_method.method_name
            )
            .unwrap();
        }

        writeln!(out, "   |").unwrap();
        let parent_throws: Vec<_> =
            violation.parent_method.declared_throws.iter().map(|d| d.error_type.as_str()).collect();
        if parent_throws.is_empty() {
            writeln!(out, "   = parent declares: (no throws allowed)").unwrap();
        } else {
            writeln!(out, "   = parent declares: @throws {{{}}}", parent_throws.join(", "))
                .unwrap();
        }
        writeln!(
            out,
            "   = help: handle the exception in the implementation or add @throws to the parent"
        )
        .unwrap();
        writeln!(out).unwrap();
    }

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();
    let violation_count = lsp_violations.len();
    writeln!(
        out,
        "Found {error_count} errors, {violation_count} LSP violations in {files_checked} files"
    )
    .unwrap();

    out
}

// 1つの PropagatedThrow を「発生元 throw + 呼び出し元へ遡るホップ列」として描画する。
// path は診断対象関数を先頭に呼び出し先へ向かって並ぶため、発生元に近い側から
// 表示するには逆順にたどる。
fn render_propagation_block(
    out: &mut String,
    missing: &PropagatedThrow,
    loader: &mut impl FnMut(&Path) -> Option<String>,
) {
    let type_name = missing.error_type.type_name().unwrap_or("Unknown");
    let origin_line = line_or_unknown(resolve_line(
        loader,
        &missing.origin_function.file_path,
        missing.origin.location.start,
    ));

    writeln!(out, "   | {type_name} propagates through:").unwrap();
    writeln!(
        out,
        "   |     throw at {}:{} in {}",
        missing.origin_function.file_path.display(),
        origin_line,
        missing.origin_function.name
    )
    .unwrap();

    for hop in missing.path.iter().rev() {
        let hop_line = line_or_unknown(resolve_line(loader, &hop.file_path, hop.span.start));
        writeln!(out, "   |     \u{2190} {} ({}:{})", hop.name, hop.file_path.display(), hop_line)
            .unwrap();
    }

    writeln!(out, "   |").unwrap();
}

// 型解決できなかった throw は `unknown` として提示する。空の型リスト
// `@throws {}` は構文として成立せず、利用者が修正手段に辿り着けないため
fn help_types(missing: &[throw_trace_core::PropagatedThrow]) -> String {
    let mut types: Vec<&str> =
        missing.iter().map(|m| m.error_type.type_name().unwrap_or("unknown")).collect();
    types.dedup();
    types.join(", ")
}

fn report_json(
    diagnostics: &[Diagnostic],
    lsp_violations: &[LspViolation],
    files_checked: usize,
) -> io::Result<()> {
    let mut loader = fs_source_loader();

    let json_diagnostics: Vec<JsonDiagnostic> =
        diagnostics.iter().map(|d| build_diagnostic_json(d, &mut loader)).collect();

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
    println!("{json}");

    Ok(())
}

fn build_diagnostic_json(
    diag: &Diagnostic,
    loader: &mut impl FnMut(&Path) -> Option<String>,
) -> JsonDiagnostic {
    JsonDiagnostic {
        file: diag.function.file_path.display().to_string(),
        function: diag.function.name.to_string(),
        missing_throws: diag
            .missing_throws
            .iter()
            .map(|m| build_missing_throw_json(m, loader))
            .collect(),
    }
}

fn build_missing_throw_json(
    missing: &PropagatedThrow,
    loader: &mut impl FnMut(&Path) -> Option<String>,
) -> JsonMissingThrow {
    let origin_line =
        resolve_line(loader, &missing.origin_function.file_path, missing.origin.location.start);

    JsonMissingThrow {
        error_type: missing.error_type.type_name().unwrap_or("Unknown").to_string(),
        origin_file: missing.origin_function.file_path.display().to_string(),
        origin_line,
        origin_function: missing.origin_function.name.to_string(),
        path: missing
            .path
            .iter()
            .map(|hop| JsonPathEntry {
                file: hop.file_path.display().to_string(),
                line: resolve_line(loader, &hop.file_path, hop.span.start),
                function: hop.name.to_string(),
            })
            .collect(),
    }
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

    // 各行 "Lx\n" は3バイトなので、offset = (line-1)*3 がその行の先頭になる。
    fn line_fixture_source() -> &'static str {
        "L1\nL2\nL3\nL4\n"
    }

    fn loader_with(sources: Vec<(&str, &str)>) -> impl FnMut(&Path) -> Option<String> {
        let map: HashMap<PathBuf, String> =
            sources.into_iter().map(|(k, v)| (PathBuf::from(k), v.to_string())).collect();
        move |path: &Path| map.get(path).cloned()
    }

    fn diagnostic_with(function: FunctionId, missing_throws: Vec<PropagatedThrow>) -> Diagnostic {
        Diagnostic { function, missing_throws }
    }

    #[test]
    fn render_text_no_issues_reports_files_checked() {
        let mut loader = loader_with(vec![]);
        let rendered = render_text(&[], &[], 3, &mut loader);
        assert_eq!(rendered, "No issues found in 3 files\n");
    }

    #[test]
    fn render_text_header_shows_resolved_line_and_function_name() {
        let mut loader = loader_with(vec![("diag.ts", line_fixture_source())]);
        let func = FunctionId::new(
            PathBuf::from("diag.ts"),
            "createTestTarget",
            Span { start: 6, end: 8 },
        );
        let missing = vec![PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 0, end: 2 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("diag.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path: vec![],
        }];
        let diag = diagnostic_with(func, missing);
        let rendered = render_text(std::slice::from_ref(&diag), &[], 1, &mut loader);

        assert!(
            rendered.contains("  --> diag.ts:3 (createTestTarget)"),
            "expected resolved header line, got: {rendered}"
        );
    }

    #[test]
    fn render_text_shows_throw_at_line_with_no_hops_when_path_empty() {
        let mut loader = loader_with(vec![("diag.ts", line_fixture_source())]);
        let func =
            FunctionId::new(PathBuf::from("diag.ts"), "validateInput", Span { start: 0, end: 2 });
        let missing = vec![PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 6, end: 8 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("diag.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path: vec![],
        }];
        let diag = diagnostic_with(func, missing);
        let rendered = render_text(std::slice::from_ref(&diag), &[], 1, &mut loader);

        assert!(
            rendered.contains(
                "   | E propagates through:\n   |     throw at diag.ts:3 in validateInput\n   |\n"
            ),
            "expected throw-at line with no hops, got: {rendered}"
        );
    }

    #[test]
    fn render_text_reverses_path_from_origin_toward_diagnostic_function() {
        let mut loader = loader_with(vec![
            ("errors.ts", line_fixture_source()),
            ("config.ts", line_fixture_source()),
            ("target.ts", line_fixture_source()),
        ]);
        // path[0] = 診断対象関数, path[1] = 呼び出し先(origin に近い側)
        let path = vec![
            FunctionId::new(
                PathBuf::from("target.ts"),
                "createTestTarget",
                Span { start: 0, end: 2 },
            ),
            FunctionId::new(PathBuf::from("config.ts"), "parseConfig", Span { start: 3, end: 5 }),
        ];
        let missing = vec![PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 6, end: 8 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("errors.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path,
        }];
        let func = FunctionId::new(
            PathBuf::from("target.ts"),
            "createTestTarget",
            Span { start: 0, end: 2 },
        );
        let diag = diagnostic_with(func, missing);
        let rendered = render_text(std::slice::from_ref(&diag), &[], 1, &mut loader);

        let expected_block = [
            "   | E propagates through:",
            "   |     throw at errors.ts:3 in validateInput",
            "   |     \u{2190} parseConfig (config.ts:2)",
            "   |     \u{2190} createTestTarget (target.ts:1)",
            "   |\n",
        ]
        .join("\n");
        assert!(
            rendered.contains(&expected_block),
            "expected reversed hop order from origin to diagnostic function, got: {rendered}"
        );
    }

    #[test]
    fn render_text_falls_back_to_question_mark_when_source_unreadable() {
        // origin_function のファイルをローダーに含めず、読み込み失敗をシミュレートする
        let mut loader = loader_with(vec![]);
        let func = FunctionId::new(
            PathBuf::from("diag.ts"),
            "createTestTarget",
            Span { start: 0, end: 2 },
        );
        let missing = vec![PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 0, end: 2 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("missing.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path: vec![],
        }];
        let diag = diagnostic_with(func, missing);
        let rendered = render_text(std::slice::from_ref(&diag), &[], 1, &mut loader);

        assert!(
            rendered.contains("  --> diag.ts:? (createTestTarget)"),
            "expected header line fallback, got: {rendered}"
        );
        assert!(
            rendered.contains("throw at missing.ts:? in validateInput"),
            "expected origin line fallback, got: {rendered}"
        );
    }

    #[test]
    fn render_text_emits_separate_blocks_for_each_propagation_without_dedup() {
        let mut loader = loader_with(vec![("diag.ts", line_fixture_source())]);
        let func = FunctionId::new(
            PathBuf::from("diag.ts"),
            "createTestTarget",
            Span { start: 0, end: 2 },
        );
        // 同じ型・同じ発生元でも経路が異なる2つの PropagatedThrow はdedupされず両方出る
        let missing = vec![
            PropagatedThrow {
                error_type: ErrorType::Named("E".into()),
                origin: ThrowSite {
                    location: Span { start: 0, end: 2 },
                    error_type: ErrorType::Named("E".into()),
                },
                origin_function: FunctionId::new(
                    PathBuf::from("diag.ts"),
                    "validateInput",
                    Span { start: 0, end: 2 },
                ),
                path: vec![FunctionId::new(
                    PathBuf::from("diag.ts"),
                    "pathA",
                    Span { start: 0, end: 2 },
                )],
            },
            PropagatedThrow {
                error_type: ErrorType::Named("E".into()),
                origin: ThrowSite {
                    location: Span { start: 0, end: 2 },
                    error_type: ErrorType::Named("E".into()),
                },
                origin_function: FunctionId::new(
                    PathBuf::from("diag.ts"),
                    "validateInput",
                    Span { start: 0, end: 2 },
                ),
                path: vec![FunctionId::new(
                    PathBuf::from("diag.ts"),
                    "pathB",
                    Span { start: 0, end: 2 },
                )],
            },
        ];
        let diag = diagnostic_with(func, missing);
        let rendered = render_text(std::slice::from_ref(&diag), &[], 1, &mut loader);

        assert_eq!(
            rendered.matches("E propagates through:").count(),
            2,
            "expected both propagation paths to be rendered, got: {rendered}"
        );
        assert!(rendered.contains("pathA"));
        assert!(rendered.contains("pathB"));
    }

    #[test]
    fn build_missing_throw_json_includes_origin_and_path() {
        let mut loader = loader_with(vec![
            ("errors.ts", line_fixture_source()),
            ("config.ts", line_fixture_source()),
        ]);
        let missing = PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 6, end: 8 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("errors.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path: vec![FunctionId::new(
                PathBuf::from("config.ts"),
                "parseConfig",
                Span { start: 3, end: 5 },
            )],
        };

        let json = build_missing_throw_json(&missing, &mut loader);

        assert_eq!(json.origin_file, "errors.ts");
        assert_eq!(json.origin_line, Some(3));
        assert_eq!(json.origin_function, "validateInput");
        assert_eq!(json.path.len(), 1);
        assert_eq!(json.path[0].file, "config.ts");
        assert_eq!(json.path[0].line, Some(2));
        assert_eq!(json.path[0].function, "parseConfig");
    }

    #[test]
    fn build_missing_throw_json_uses_null_line_when_source_unreadable() {
        let mut loader = loader_with(vec![]);
        let missing = PropagatedThrow {
            error_type: ErrorType::Named("E".into()),
            origin: ThrowSite {
                location: Span { start: 0, end: 2 },
                error_type: ErrorType::Named("E".into()),
            },
            origin_function: FunctionId::new(
                PathBuf::from("missing.ts"),
                "validateInput",
                Span { start: 0, end: 2 },
            ),
            path: vec![],
        };

        let json = build_missing_throw_json(&missing, &mut loader);

        assert_eq!(json.origin_line, None);
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(serialized.contains("\"origin_line\":null"));
    }
}
