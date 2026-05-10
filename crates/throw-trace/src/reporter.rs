use serde::Serialize;
use std::io::{self, Write};
use throw_trace_core::Diagnostic;

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
    origin_line: u32,
}

#[derive(Serialize)]
struct Summary {
    errors: usize,
    files_checked: usize,
}

pub fn report(
    diagnostics: &[Diagnostic],
    files_checked: usize,
    format: OutputFormat,
) -> io::Result<()> {
    match format {
        OutputFormat::Text => report_text(diagnostics, files_checked),
        OutputFormat::Json => report_json(diagnostics, files_checked),
    }
}

fn report_text(diagnostics: &[Diagnostic], files_checked: usize) -> io::Result<()> {
    let mut stdout = io::stdout().lock();

    if diagnostics.is_empty() {
        writeln!(stdout, "No issues found in {files_checked} files")?;
        return Ok(());
    }

    for diag in diagnostics {
        writeln!(stdout, "error: missing @throws declaration")?;
        writeln!(stdout, "  --> {}:{}", diag.function.file_path.display(), diag.function.name)?;
        writeln!(stdout, "   |")?;

        for missing in &diag.missing_throws {
            let type_name = missing.error_type.type_name().unwrap_or("Unknown");
            writeln!(stdout, "   | {} propagates from {:?}", type_name, missing.origin.location)?;
        }

        writeln!(stdout, "   |")?;
        let types: Vec<_> =
            diag.missing_throws.iter().filter_map(|m| m.error_type.type_name()).collect();
        writeln!(
            stdout,
            "   = help: add @throws {{{}}} to function {}",
            types.join(", "),
            diag.function.name
        )?;
        writeln!(stdout)?;
    }

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();
    writeln!(stdout, "Found {error_count} errors in {files_checked} files")?;

    Ok(())
}

fn report_json(diagnostics: &[Diagnostic], files_checked: usize) -> io::Result<()> {
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
                    origin_file: String::new(),
                    origin_line: m.origin.location.start,
                })
                .collect(),
        })
        .collect();

    let error_count: usize = diagnostics.iter().map(|d| d.missing_throws.len()).sum();

    let report = JsonReport {
        diagnostics: json_diagnostics,
        summary: Summary { errors: error_count, files_checked },
    };

    let json = serde_json::to_string_pretty(&report)?;
    println!("{json}");

    Ok(())
}
