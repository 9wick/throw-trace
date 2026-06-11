mod analyzer;
mod cache;
mod fixer;
mod loader;
mod reporter;

use analyzer::Analyzer;
use anyhow::Result;
use clap::Parser;
use fixer::fix_files;
use loader::FileLoader;
use reporter::{report, OutputFormat};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "throw-trace")]
#[command(about = "Static analysis tool for @throws TSDoc declarations")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Check TypeScript files for missing @throws declarations
    Check {
        /// Files or directories to check
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Exclude patterns (glob)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,

        /// Output format (text or json)
        #[arg(long, short = 'f', default_value = "text")]
        format: String,
    },
    /// Auto-insert missing @throws declarations
    Fix {
        /// Files or directories to fix
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Exclude patterns (glob)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,
    },
}

// exit code 規約: 0 = 違反なし, 1 = 違反検出, 2 = 実行時エラー
// (clippy/eslint と同様、CI が「違反あり」と「ツール故障」を区別できるようにする)
fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { paths, exclude, format }) => {
            let output_format = OutputFormat::from_str(&format)
                .ok_or_else(|| anyhow::anyhow!("Invalid format: {format}"))?;

            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(ExitCode::SUCCESS);
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();
            let lsp_violations = analyzer.generate_lsp_violations();

            report(&diagnostics, &lsp_violations, files.len(), output_format)?;

            if !diagnostics.is_empty() || !lsp_violations.is_empty() {
                return Ok(ExitCode::from(1));
            }

            Ok(ExitCode::SUCCESS)
        }
        Some(Commands::Fix { paths, exclude }) => {
            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(ExitCode::SUCCESS);
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();
            let fixed_count = fix_files(&diagnostics)?;

            println!("Fixed {fixed_count} file(s)");
            Ok(ExitCode::SUCCESS)
        }
        None => {
            println!("Use --help for usage information");
            Ok(ExitCode::SUCCESS)
        }
    }
}
