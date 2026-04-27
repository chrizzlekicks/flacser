use std::{num::NonZeroUsize, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "flacser")]
#[command(about = "Convert .flac files to .aiff with ffmpeg")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Convert(ConvertArgs),
}

#[derive(Debug, clap::Args)]
pub struct ConvertArgs {
    // Input file or directory
    pub input_path: PathBuf,

    // Output directory
    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    // Overwrite existing outputs
    #[arg(long, default_value_t = false)]
    pub overwrite: bool,

    // Dry run (no exec)
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    // Recurse into subdirectories in directory mode
    #[arg(long, default_value_t = false)]
    pub recursive: bool,

    // Number of parallel conversion jobs
    #[arg(long)]
    pub jobs: Option<NonZeroUsize>,
}

pub fn parse() -> Cli {
    Cli::parse()
}
