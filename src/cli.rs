use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about =Some("Make a folder reproducible by specifying its contents in a file."))]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Sync to a specified directory")]
    Sync {
        #[arg(help = "Config file")]
        file: PathBuf,
        #[arg(help = "Destination folder")]
        output: PathBuf,
        #[arg(short, long)]
        tags: Vec<String>,
    },
    #[command(
        about = "Print a report. Fails if a file has no valid sources or a hash does not match."
    )]
    Check {
        #[arg(help = "Config file")]
        file: PathBuf,
    },
    #[command(about = "Writes out an example file")]
    Example {},
    #[command(about = "Prints the sha3-256 hash of a file.")]
    Hash { file: PathBuf },
}
