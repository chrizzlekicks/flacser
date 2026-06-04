use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::plan::ConversionJob;

const FFMPEG_NOT_FOUND: &str = "ffmpeg not found.\n\nInstall it with:\n  Arch:   sudo pacman -S ffmpeg\n  Ubuntu: sudo apt install ffmpeg\n  macOS:  brew install ffmpeg";

#[derive(Debug, Clone)]
pub struct VersionProbe {
    pub executable_found: bool,
    pub version: std::result::Result<String, String>,
}

pub fn check_availability() -> Result<()> {
    match read_version() {
        Ok(_) => Ok(()),
        Err(_) => bail!(FFMPEG_NOT_FOUND),
    }
}

pub fn read_version() -> Result<String> {
    read_version_output()
        .map(|output| output.version)
        .map_err(|error| anyhow::anyhow!(error.message))
}

pub fn probe_version() -> VersionProbe {
    match read_version_output() {
        Ok(output) => VersionProbe {
            executable_found: true,
            version: Ok(output.version),
        },
        Err(error) => VersionProbe {
            executable_found: !error.spawn_failed,
            version: Err(error.message),
        },
    }
}

struct VersionOutput {
    version: String,
}

struct VersionError {
    spawn_failed: bool,
    message: String,
}

fn read_version_output() -> std::result::Result<VersionOutput, VersionError> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| VersionError {
            spawn_failed: true,
            message: format!("failed to run ffmpeg -version: {error}"),
        })?;

    if !output.status.success() {
        return Err(VersionError {
            spawn_failed: false,
            message: format!("ffmpeg -version exited with status {}", output.status),
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| VersionError {
        spawn_failed: false,
        message: format!("ffmpeg version output is not UTF-8: {error}"),
    })?;
    let first_line = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| VersionError {
            spawn_failed: false,
            message: "ffmpeg version output was empty".to_string(),
        })?;

    Ok(VersionOutput {
        version: first_line.to_string(),
    })
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
