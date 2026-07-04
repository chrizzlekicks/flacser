use std::{num::NonZeroUsize, path::PathBuf};

use clap::{Args, Parser, Subcommand};

use crate::audio_format::AudioFormat;

#[derive(Debug, Parser)]
#[command(
    name = "flacser",
    about = "Convert lossless audio files with ffmpeg",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Convert FLAC, AIFF, or WAV files to a target lossless format.
    Convert(ConvertArgs),

    /// Check whether the system is ready to run conversions.
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct ConvertArgs {
    /// Input audio file or directory to convert. Source format is inferred from extension.
    pub input_path: PathBuf,

    /// Target format: flac, aiff, or wav. Falls back to FLACSER_CONVERT_TO.
    #[arg(long, value_enum, env = "FLACSER_CONVERT_TO")]
    pub to: Option<AudioFormat>,

    /// Write converted files into this directory.
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

    /// Target format: flac, aiff, or wav. Falls back to FLACSER_CONVERT_TO.
    #[arg(long, value_enum, env = "FLACSER_CONVERT_TO")]
    pub to: Option<AudioFormat>,

    /// Diagnose this output directory without creating it.
    #[arg(short = 'o', long)]
    pub output_dir: Option<PathBuf>,

    /// Diagnose this parallel conversion job limit.
    #[arg(short = 'j', long)]
    pub jobs: Option<NonZeroUsize>,
}

pub fn parse() -> Cli {
    Cli::parse()
}
