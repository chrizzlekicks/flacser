use std::{num::NonZeroUsize, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "flacser",
    about = "Convert .flac files to .aiff with ffmpeg",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Convert .flac file or directory with multiple .flac files to .aiff.
    Convert(ConvertArgs),

    /// Check whether the system is ready to run conversions.
    Doctor(DoctorArgs),
}

#[derive(Debug, clap::Args)]
pub struct ConvertArgs {
    /// Input `.flac` file or directory to convert.
    pub input_path: PathBuf,

    /// Write converted `.aiff` files into this directory.
    #[arg(short = 'o', long)]
    pub output_dir: Option<PathBuf>,

    /// Replace existing output files instead of skipping them.
    #[arg(short = 'w', long, default_value_t = false)]
    pub overwrite: bool,

    /// Print the conversion plan without running `ffmpeg`.
    #[arg(short = 'n', long, default_value_t = false)]
    pub dry_run: bool,

    /// Recurse into subdirectories when the input path is a directory.
    #[arg(short = 'r', long, default_value_t = false)]
    pub recursive: bool,

    /// Limit the number of parallel conversion jobs.
    #[arg(short = 'j', long)]
    pub jobs: Option<NonZeroUsize>,
}

#[derive(Debug, clap::Args)]
pub struct DoctorArgs {
    /// Optional input path to diagnose before conversion.
    pub input_path: Option<PathBuf>,
}

pub fn parse() -> Cli {
    Cli::parse()
}
