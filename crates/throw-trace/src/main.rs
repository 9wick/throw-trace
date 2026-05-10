use clap::Parser;

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
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { paths }) => {
            println!("Checking: {:?}", paths);
        }
        None => {
            println!("Use --help for usage information");
        }
    }
}
