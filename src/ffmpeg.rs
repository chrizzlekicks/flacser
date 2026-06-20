use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};

#[cfg(test)]
use std::ffi::OsString;

use anyhow::{Context, Result, bail};

use crate::plan::ConversionJob;
use crate::{audio_format::AudioFormat, config::Config};

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

#[cfg(windows)]
fn ffmpeg_command_candidates() -> &'static [&'static str] {
    &["ffmpeg", "ffmpeg.cmd"]
}

#[cfg(not(windows))]
fn ffmpeg_command_candidates() -> &'static [&'static str] {
    &["ffmpeg"]
}

fn run_ffmpeg_candidate<T>(run: impl Fn(&str) -> io::Result<T>) -> io::Result<T> {
    let mut spawn_error = None;

    for candidate in ffmpeg_command_candidates() {
        match run(candidate) {
            Ok(result) => return Ok(result),
            Err(error) => spawn_error = Some(error),
        }
    }

    Err(spawn_error.expect("at least one ffmpeg candidate"))
}

fn read_version_output() -> std::result::Result<VersionOutput, VersionError> {
    let output = run_ffmpeg_candidate(|candidate| {
        Command::new(candidate)
            .arg("-version")
            .stdin(Stdio::null())
            .output()
    })
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

pub fn run_ffmpeg(job: &ConversionJob, output: &Path) -> Result<()> {
    debug_assert_ne!(job.source_format, job.target_format);

    let status = run_ffmpeg_candidate(|candidate| ffmpeg_command(candidate, job, output).status())
        .with_context(|| format!("failed to spawn ffmpeg for {}", job.input.display()))?;

    if !status.success() {
        bail!(
            "ffmpeg exited with status {} for input {}",
            status,
            job.input.display()
        );
    }

    Ok(())
}

fn ffmpeg_command(candidate: &str, job: &ConversionJob, output: &Path) -> Command {
    let mut command = Command::new(candidate);
    command.arg("-nostdin").arg("-i").arg(&job.input);

    match job.target_format {
        AudioFormat::Aiff => {
            command
                .arg("-map")
                .arg("0")
                .arg("-c:a")
                .arg("pcm_s16be")
                .arg("-write_id3v2")
                .arg("1");
        }
        AudioFormat::Flac => {
            command.arg("-map").arg("0").arg("-c:a").arg("flac");
        }
        AudioFormat::Wav => {
            command
                .arg("-map")
                .arg("0:a:0")
                .arg("-map_metadata")
                .arg("0")
                .arg("-c:a")
                .arg("pcm_s16le");
        }
    }

    command.arg("-y").arg("-loglevel").arg("error").arg(output);
    command
}

#[cfg(test)]
fn command_args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(OsString::from)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::{audio_format::AudioFormat, plan::ConversionJob};

    use super::{command_args, ffmpeg_command};

    fn job(target_format: AudioFormat) -> ConversionJob {
        ConversionJob {
            input: PathBuf::from("input.flac"),
            output: PathBuf::from("output"),
            source_format: AudioFormat::Flac,
            target_format,
        }
    }

    #[test]
    fn builds_flac_codec_args() {
        let command = ffmpeg_command("ffmpeg", &job(AudioFormat::Flac), Path::new("out.flac"));
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "flac"]));
        assert!(!args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.flac"));
    }

    #[test]
    fn builds_aiff_args_with_pcm_s16be() {
        let command = ffmpeg_command("ffmpeg", &job(AudioFormat::Aiff), Path::new("out.aiff"));
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "pcm_s16be"]));
        assert!(args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.aiff"));
    }

    #[test]
    fn builds_wav_args_with_pcm_s16le_and_audio_only_mapping() {
        let command = ffmpeg_command("ffmpeg", &job(AudioFormat::Wav), Path::new("out.wav"));
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0:a:0"]));
        assert!(args.windows(2).any(|args| args == ["-map_metadata", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "pcm_s16le"]));
        assert!(!args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(!args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.wav"));
    }
}
