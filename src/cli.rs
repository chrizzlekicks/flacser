use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "flacser")]
#[command(about = "Convert .flac files to .aiff with ffmpeg")]
pub struct Cli {
    // Input file or directory
    pub input_path: PathBuf,

    // Output directory
    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    // Dry run (no exec)
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
