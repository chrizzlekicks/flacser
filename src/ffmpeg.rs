use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::plan::ConversionJob;

const FFMPEG_NOT_FOUND: &str = "ffmpeg not found.\n\nInstall it with:\n  Arch:   sudo pacman -S ffmpeg\n  Ubuntu: sudo apt install ffmpeg\n  macOS:  brew install ffmpeg";

pub fn check_availability() -> Result<()> {
    let status = Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let status = match status {
        Ok(status) => status,
        Err(_) => bail!(FFMPEG_NOT_FOUND),
    };

    if !status.success() {
        bail!(FFMPEG_NOT_FOUND);
    }

    Ok(())
}

pub fn is_needed(config: &Config, jobs: &[ConversionJob]) -> bool {
    !config.dry_run
        && jobs
            .iter()
            .any(|job| config.overwrite || !job.output.exists())
}

pub fn run_ffmpeg(input: &Path, output: &Path) -> Result<()> {
    let status = Command::new("ffmpeg")
        .arg("-nostdin")
        .arg("-i")
        .arg(input)
        .arg("-map")
        .arg("0")
        .arg("-write_id3v2")
        .arg("1")
        .arg("-y")
        .arg("-loglevel")
        .arg("error")
        .arg(output)
        .status()
        .with_context(|| format!("failed to spawn ffmpeg for {}", input.display()))?;

    if !status.success() {
        bail!(
            "ffmpeg exited with status {} for input {}",
            status,
            input.display()
        );
    }

    Ok(())
}
