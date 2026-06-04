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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { paths, exclude, format }) => {
            let output_format = OutputFormat::from_str(&format)
                .ok_or_else(|| anyhow::anyhow!("Invalid format: {format}"))?;

            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(());
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();
            let lsp_violations = analyzer.generate_lsp_violations();

            report(&diagnostics, &lsp_violations, files.len(), output_format)?;

            if !diagnostics.is_empty() || !lsp_violations.is_empty() {
                std::process::exit(1);
            }

            Ok(())
        }
        Some(Commands::Fix { paths, exclude }) => {
            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;

            if files.is_empty() {
                println!("No TypeScript files found");
                return Ok(());
            }

            let mut analyzer = Analyzer::new();
            analyzer.analyze_files(&files)?;

            let diagnostics = analyzer.generate_diagnostics();
            let fixed_count = fix_files(&diagnostics)?;

            println!("Fixed {fixed_count} file(s)");
            Ok(())
        }
        None => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}
