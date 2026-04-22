use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};

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
