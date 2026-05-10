mod loader;

use anyhow::Result;
use clap::Parser;
use loader::FileLoader;

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

        /// Output format
        #[arg(long, short = 'f', default_value = "text")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check {
            paths,
            exclude,
            format,
        }) => {
            let loader = FileLoader::new(&exclude)?;
            let files = loader.collect_ts_files(&paths)?;
            println!("Found {} TypeScript files", files.len());
            for file in &files {
                println!("  {}", file.display());
            }
            Ok(())
        }
        None => {
            println!("Use --help for usage information");
            Ok(())
        }
    }
}
