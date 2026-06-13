//! Communication with TypeScript's tsserver for semantic type information.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TsServerError {
    #[error("Failed to spawn tsserver: {0}")]
    SpawnFailed(#[from] std::io::Error),
    #[error("tsserver not found. Install TypeScript: npm install -g typescript")]
    NotFound,
    #[error("Failed to parse response: {0}")]
    ParseError(#[from] serde_json::Error),
    #[error("tsserver returned error: {0}")]
    ServerError(String),
}

#[derive(Serialize)]
struct Request<'a> {
    seq: u32,
    #[serde(rename = "type")]
    msg_type: &'static str,
    command: &'a str,
    arguments: serde_json::Value,
}

#[derive(Deserialize, Debug)]
struct Response {
    #[serde(rename = "type")]
    msg_type: String,
    #[allow(dead_code)]
    command: Option<String>,
    request_seq: Option<u32>,
    success: Option<bool>,
    body: Option<serde_json::Value>,
    message: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FileSpan {
    pub file: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Position {
    pub line: u32,
    pub offset: u32,
}

fn read_response_from<R: BufRead>(
    reader: &mut R,
    expected_seq: u32,
) -> Result<Response, TsServerError> {
    loop {
        let mut line = String::new();
        // EOF（tsserver クラッシュ等）を continue で握りつぶすと無限ビジーループになる
        if reader.read_line(&mut line)? == 0 {
            return Err(TsServerError::ServerError(
                "tsserver closed stdout unexpectedly (EOF)".to_string(),
            ));
        }

        let line = line.trim();
        if line.is_empty() || line.starts_with("Content-Length:") {
            continue;
        }

        let response: Response = serde_json::from_str(line)?;

        if response.msg_type == "response" && response.request_seq == Some(expected_seq) {
            if response.success == Some(false) {
                return Err(TsServerError::ServerError(
                    response.message.unwrap_or_else(|| "Unknown error".to_string()),
                ));
            }
            return Ok(response);
        }
    }
}

pub struct TsServer {
    process: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    seq: AtomicU32,
}

impl TsServer {
    pub fn new() -> Result<Self, TsServerError> {
        let tsserver_path = which_tsserver()?;

        let mut process = Command::new(tsserver_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = process.stdin.take().expect("stdin should be piped");
        let stdout = BufReader::new(process.stdout.take().expect("stdout should be piped"));

        Ok(Self { process, stdin, stdout, seq: AtomicU32::new(0) })
    }

    fn next_seq(&self) -> u32 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn send_request(
        &mut self,
        command: &str,
        arguments: serde_json::Value,
    ) -> Result<u32, TsServerError> {
        let seq = self.next_seq();
        let request = Request { seq, msg_type: "request", command, arguments };

        let json = serde_json::to_string(&request)?;
        writeln!(self.stdin, "{json}")?;
        self.stdin.flush()?;

        Ok(seq)
    }

    fn read_response(&mut self, expected_seq: u32) -> Result<Response, TsServerError> {
        read_response_from(&mut self.stdout, expected_seq)
    }

    pub fn open_file(&mut self, file_path: &Path) -> Result<(), TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "open",
            serde_json::json!({
                "file": abs_path.to_string_lossy()
            }),
        )?;
        self.read_response(seq)?;
        Ok(())
    }

    /// Get the type definition locations for a symbol at the given position.
    /// For union types, returns multiple locations (one per constituent type).
    pub fn type_definition(
        &mut self,
        file_path: &Path,
        line: u32,
        offset: u32,
    ) -> Result<Vec<FileSpan>, TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "typeDefinition",
            serde_json::json!({
                "file": abs_path.to_string_lossy(),
                "line": line,
                "offset": offset
            }),
        )?;

        let response = self.read_response(seq)?;

        if let Some(body) = response.body {
            let spans: Vec<FileSpan> = serde_json::from_value(body)?;
            Ok(spans)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get the definition locations for a symbol at the given position.
    /// Returns the file and position where the symbol is defined.
    pub fn definition(
        &mut self,
        file_path: &Path,
        line: u32,
        offset: u32,
    ) -> Result<Vec<FileSpan>, TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "definition",
            serde_json::json!({
                "file": abs_path.to_string_lossy(),
                "line": line,
                "offset": offset
            }),
        )?;

        let response = self.read_response(seq)?;

        if let Some(body) = response.body {
            let spans: Vec<FileSpan> = serde_json::from_value(body)?;
            Ok(spans)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get quick info (hover) for a symbol at the given position.
    pub fn quick_info(
        &mut self,
        file_path: &Path,
        line: u32,
        offset: u32,
    ) -> Result<Option<String>, TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "quickinfo",
            serde_json::json!({
                "file": abs_path.to_string_lossy(),
                "line": line,
                "offset": offset
            }),
        )?;

        let response = self.read_response(seq)?;

        if let Some(body) = response.body {
            if let Some(display) = body.get("displayString") {
                return Ok(display.as_str().map(String::from));
            }
        }
        Ok(None)
    }

    /// Update file content virtually (without writing to disk).
    pub fn update_open(
        &mut self,
        file_path: &Path,
        line: u32,
        offset: u32,
        new_text: &str,
    ) -> Result<(), TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "updateOpen",
            serde_json::json!({
                "changedFiles": [{
                    "fileName": abs_path.to_string_lossy(),
                    "textChanges": [{
                        "start": { "line": line, "offset": offset },
                        "end": { "line": line, "offset": offset },
                        "newText": new_text
                    }]
                }]
            }),
        )?;
        self.read_response(seq)?;
        Ok(())
    }

    /// Get semantic diagnostics (type errors) for a file.
    pub fn semantic_diagnostics(
        &mut self,
        file_path: &Path,
    ) -> Result<Vec<serde_json::Value>, TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request(
            "semanticDiagnosticsSync",
            serde_json::json!({
                "file": abs_path.to_string_lossy()
            }),
        )?;

        let response = self.read_response(seq)?;

        if let Some(body) = response.body {
            if let Some(arr) = body.as_array() {
                return Ok(arr.clone());
            }
        }
        Ok(Vec::new())
    }

    /// Reload file from disk (discard virtual changes).
    pub fn reload_file(&mut self, file_path: &Path) -> Result<(), TsServerError> {
        let abs_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let seq = self.send_request("reloadProjects", serde_json::json!({}))?;
        self.read_response(seq)?;

        let seq =
            self.send_request("close", serde_json::json!({ "file": abs_path.to_string_lossy() }))?;
        self.read_response(seq)?;

        let seq =
            self.send_request("open", serde_json::json!({ "file": abs_path.to_string_lossy() }))?;
        self.read_response(seq)?;
        Ok(())
    }
}

impl Drop for TsServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

/// Type resolver implementation using tsserver.
pub struct TsServerTypeResolver {
    server: TsServer,
    opened_files: std::collections::HashSet<std::path::PathBuf>,
    check_cache: std::collections::HashMap<(std::path::PathBuf, String, String), bool>,
}

impl TsServerTypeResolver {
    pub fn new() -> Result<Self, TsServerError> {
        Ok(Self {
            server: TsServer::new()?,
            opened_files: std::collections::HashSet::new(),
            check_cache: std::collections::HashMap::new(),
        })
    }

    fn ensure_file_open(&mut self, file_path: &Path) -> Result<(), TsServerError> {
        let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        if !self.opened_files.contains(&canonical) {
            self.server.open_file(file_path)?;
            self.opened_files.insert(canonical);
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn count_lines(file_path: &Path) -> u32 {
        std::fs::read_to_string(file_path).map(|s| s.lines().count() as u32).unwrap_or(1)
    }
}

/// Convert byte offset to (line, column) for tsserver.
/// Line and column are 1-based.
#[allow(clippy::cast_possible_truncation)]
pub fn byte_offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;

    for (i, ch) in source.char_indices() {
        if i as u32 >= offset {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

impl throw_trace_core::TypeResolver for TsServerTypeResolver {
    fn is_assignable_to(
        &mut self,
        file_path: &std::path::Path,
        thrown_type: &str,
        declared_type: &str,
    ) -> bool {
        if thrown_type == declared_type {
            return true;
        }

        let canonical_file = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
        let cache_key = (canonical_file, thrown_type.to_string(), declared_type.to_string());
        if let Some(&result) = self.check_cache.get(&cache_key) {
            return result;
        }

        if self.ensure_file_open(file_path).is_err() {
            return thrown_type == declared_type;
        }

        let line_count = Self::count_lines(file_path);
        let check_line = line_count + 1;
        let check_code = format!("const _typeCheck: {declared_type} = null! as {thrown_type};\n");

        if self.server.update_open(file_path, check_line, 1, &check_code).is_err() {
            return thrown_type == declared_type;
        }

        let result = match self.server.semantic_diagnostics(file_path) {
            Ok(diagnostics) => diagnostics.is_empty(),
            Err(_) => thrown_type == declared_type,
        };

        let _ = self.server.reload_file(file_path);

        self.check_cache.insert(cache_key, result);
        result
    }

    #[allow(clippy::cast_possible_truncation)]
    fn resolve_type(
        &mut self,
        file_path: &std::path::Path,
        span: throw_trace_core::Span,
    ) -> Option<String> {
        self.ensure_file_open(file_path).ok()?;

        let source = std::fs::read_to_string(file_path).ok()?;
        let throw_text = source.get(span.start as usize..span.end as usize)?;

        if !throw_text.starts_with("throw ") {
            return None;
        }

        let expr = throw_text.strip_prefix("throw ")?.trim_end_matches(';').trim();

        // For call expressions like `Foo.bar(args)`, query at the method name position
        // to get the return type. For simple identifiers, query at the end.
        let query_offset = if let Some(paren_pos) = expr.find('(') {
            span.start + 6 + paren_pos as u32 - 1
        } else {
            span.start + 6 + expr.len() as u32 - 1
        };
        let (line, col) = byte_offset_to_line_col(&source, query_offset);

        let display = self.server.quick_info(file_path, line, col).ok()??;

        if let Some(colon_pos) = display.rfind(':') {
            let type_part = display[colon_pos + 1..].trim();
            return Some(type_part.to_string());
        }

        None
    }
}

fn which_tsserver() -> Result<String, TsServerError> {
    let node_modules_path = "node_modules/.bin/tsserver";
    if Path::new(node_modules_path).exists() {
        return Ok(node_modules_path.to_string());
    }

    if let Ok(output) = Command::new("which").arg("tsserver").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }

    Err(TsServerError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use throw_trace_core::TypeResolver;

    #[test]
    #[ignore = "requires tsserver installed"]
    fn test_open_and_type_definition() {
        let mut server = TsServer::new().expect("tsserver should start");

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/type_alias.ts");

        server.open_file(&fixture).expect("open should succeed");

        // Line 9: "throw err" - get type of "err"
        let spans = server.type_definition(&fixture, 9, 9).expect("typeDefinition should succeed");

        // Should return ErrorA and ErrorB definitions
        assert_eq!(spans.len(), 2, "Union type should have 2 constituents");
    }

    #[test]
    #[ignore = "requires tsserver installed"]
    fn test_is_assignable_to_same_type() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/errors.ts");

        let mut resolver = TsServerTypeResolver::new().expect("resolver should start");
        assert!(resolver.is_assignable_to(&fixture, "ErrorA", "ErrorA"));
    }

    #[test]
    #[ignore = "requires tsserver installed"]
    fn test_is_assignable_to_union() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/errors.ts");

        let mut resolver = TsServerTypeResolver::new().expect("resolver should start");
        // ErrorA is assignable to MyErrorUnion (= ErrorA | ErrorB)
        assert!(resolver.is_assignable_to(&fixture, "ErrorA", "MyErrorUnion"));
    }

    #[test]
    #[ignore = "requires tsserver installed"]
    fn test_is_not_assignable_base_to_derived() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/inheritance.ts");

        let mut resolver = TsServerTypeResolver::new().expect("resolver should start");
        // BaseError is NOT assignable to DerivedError
        assert!(!resolver.is_assignable_to(&fixture, "BaseError", "DerivedError"));
    }

    #[test]
    #[ignore = "requires tsserver installed"]
    fn test_definition_returns_cross_file() {
        let mut server = TsServer::new().expect("tsserver should start");

        let fixture_b = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/cross_file/b.ts");

        let _fixture_a = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("tests/fixtures/cross_file/a.ts");

        server.open_file(&fixture_b).expect("open should succeed");

        // Line 4: "validate(input)" - get definition of "validate"
        // Position is at the start of "validate" identifier
        let spans = server.definition(&fixture_b, 4, 3).expect("definition should succeed");

        assert!(!spans.is_empty(), "Should return at least one definition");

        let def = &spans[0];
        assert!(def.file.ends_with("a.ts"), "Definition should point to a.ts, got: {}", def.file);
    }

    #[test]
    fn read_response_returns_error_on_eof() {
        // tsserver がクラッシュ等で stdout を閉じた場合、無限ループせずエラーを返す
        let mut reader = std::io::Cursor::new("");
        let result = read_response_from(&mut reader, 1);
        assert!(result.is_err(), "EOF must produce an error, not loop forever");
    }

    #[test]
    fn read_response_returns_error_on_eof_after_events() {
        let input = "Content-Length: 76\n\n{\"seq\":0,\"type\":\"event\",\"event\":\"typingsInstallerPid\",\"body\":{\"pid\":123}}\n";
        let mut reader = std::io::Cursor::new(input);
        let result = read_response_from(&mut reader, 1);
        assert!(result.is_err(), "EOF after unrelated events must produce an error");
    }

    #[test]
    fn read_response_skips_events_and_finds_matching_response() {
        let input = "Content-Length: 40\n\n{\"seq\":0,\"type\":\"event\",\"event\":\"projectLoadingStart\"}\n{\"seq\":1,\"type\":\"response\",\"command\":\"open\",\"request_seq\":1,\"success\":true}\n";
        let mut reader = std::io::Cursor::new(input);
        let result = read_response_from(&mut reader, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_byte_offset_to_line_col() {
        let source = "line1\nline2\nline3";

        // Start of file
        assert_eq!(byte_offset_to_line_col(source, 0), (1, 1));

        // End of first line
        assert_eq!(byte_offset_to_line_col(source, 4), (1, 5));

        // Start of second line (after \n)
        assert_eq!(byte_offset_to_line_col(source, 6), (2, 1));

        // Middle of second line
        assert_eq!(byte_offset_to_line_col(source, 8), (2, 3));

        // Start of third line
        assert_eq!(byte_offset_to_line_col(source, 12), (3, 1));
    }
}
