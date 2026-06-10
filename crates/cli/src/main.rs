//! `codemap` — thin CLI over the extractor.

use std::error::Error;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use extract::extract_repo;

#[derive(Parser)]
#[command(name = "codemap", about = "Build an architecture map of a codebase")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Extract the structural IR for a repository and print canonical JSON.
    Analyze {
        /// Path to the repository root (must contain go.mod for Go).
        path: PathBuf,
        /// Write output to a file instead of stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Analyze { path, output } => {
            let doc = extract_repo(&path)?;
            let json = doc.to_canonical_json();
            match output {
                Some(file) => std::fs::write(&file, format!("{json}\n"))?,
                None => println!("{json}"),
            }
        }
    }
    Ok(())
}
