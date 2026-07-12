use std::{
    io,
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::{Context, Result, bail};

use crate::plan::ConversionJob;
use crate::{audio_format::AudioFormat, config::Config};

const FFMPEG_NOT_FOUND: &str = "ffmpeg not found.\n\nInstall it with:\n  Arch:   sudo pacman -S ffmpeg\n  Ubuntu: sudo apt install ffmpeg\n  macOS:  brew install ffmpeg";
const FFPROBE_NOT_FOUND: &str = "ffprobe not found.\n\nInstall it with:\n  Arch:   sudo pacman -S ffmpeg\n  Ubuntu: sudo apt install ffmpeg\n  macOS:  brew install ffmpeg";

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

pub fn check_probe_availability() -> Result<()> {
    let status = run_ffprobe_candidate(|candidate| {
        Command::new(candidate)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    });

    match status {
        Ok(status) if status.success() => Ok(()),
        _ => bail!(FFPROBE_NOT_FOUND),
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

pub fn probe_ffprobe_version() -> VersionProbe {
    match read_ffprobe_version_output() {
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

#[cfg(windows)]
fn ffprobe_command_candidates() -> &'static [&'static str] {
    &["ffprobe", "ffprobe.cmd"]
}

#[cfg(not(windows))]
fn ffmpeg_command_candidates() -> &'static [&'static str] {
    &["ffmpeg"]
}

#[cfg(not(windows))]
fn ffprobe_command_candidates() -> &'static [&'static str] {
    &["ffprobe"]
}

fn run_ffmpeg_candidate<T>(run: impl Fn(&str) -> io::Result<T>) -> io::Result<T> {
    run_command_candidate(ffmpeg_command_candidates(), run)
}

fn run_ffprobe_candidate<T>(run: impl Fn(&str) -> io::Result<T>) -> io::Result<T> {
    run_command_candidate(ffprobe_command_candidates(), run)
}

fn run_command_candidate<T>(
    candidates: &[&str],
    run: impl Fn(&str) -> io::Result<T>,
) -> io::Result<T> {
    let mut spawn_error = None;

    for candidate in candidates {
        match run(candidate) {
            Ok(result) => return Ok(result),
            Err(error) => spawn_error = Some(error),
        }
    }

    Err(spawn_error.expect("at least one ffmpeg candidate"))
}

fn read_version_output() -> std::result::Result<VersionOutput, VersionError> {
    read_command_version_output("ffmpeg", run_ffmpeg_candidate)
}

fn read_ffprobe_version_output() -> std::result::Result<VersionOutput, VersionError> {
    read_command_version_output("ffprobe", run_ffprobe_candidate)
}

fn read_command_version_output(
    command_name: &str,
    run_candidate: impl FnOnce(fn(&str) -> io::Result<Output>) -> io::Result<Output>,
) -> std::result::Result<VersionOutput, VersionError> {
    fn read_candidate_version(candidate: &str) -> io::Result<Output> {
        Command::new(candidate)
            .arg("-version")
            .stdin(Stdio::null())
            .output()
    }

    let output = run_candidate(read_candidate_version).map_err(|error| VersionError {
        spawn_failed: true,
        message: format!("failed to run {command_name} -version: {error}"),
    })?;

    if !output.status.success() {
        return Err(VersionError {
            spawn_failed: false,
            message: format!(
                "{command_name} -version exited with status {}",
                output.status
            ),
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| VersionError {
        spawn_failed: false,
        message: format!("{command_name} version output is not UTF-8: {error}"),
    })?;
    let first_line = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| VersionError {
            spawn_failed: false,
            message: format!("{command_name} version output was empty"),
        })?;

    Ok(VersionOutput {
        version: first_line.to_string(),
    })
}

pub fn is_needed(config: &Config, jobs: &[ConversionJob]) -> bool {
    !config.dry_run && jobs.iter().any(|job| !job.output.exists())
}

pub fn probe_is_needed(config: &Config, jobs: &[ConversionJob]) -> bool {
    !config.dry_run
        && jobs.iter().any(|job| {
            !job.output.exists()
                && matches!(job.target_format, AudioFormat::Aiff | AudioFormat::Wav)
        })
}

pub fn run_ffmpeg(job: &ConversionJob, output: &Path) -> Result<()> {
    debug_assert_ne!(job.source_format, job.target_format);

    let pcm_codec = match job.target_format {
        AudioFormat::Aiff | AudioFormat::Wav => Some(probe_pcm_codec(job)?),
        AudioFormat::Flac => None,
    };

    let status = run_ffmpeg_candidate(|candidate| {
        ffmpeg_command(candidate, job, output, pcm_codec).status()
    })
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

#[derive(Debug, PartialEq, Eq)]
struct AudioSample {
    sample_fmt: Option<String>,
    bits: Option<u16>,
}

fn probe_pcm_codec(job: &ConversionJob) -> Result<&'static str> {
    let sample = probe_audio_sample(&job.input)?;
    pcm_codec_for(job.target_format, &sample).with_context(|| {
        format!(
            "could not determine PCM bit depth for {}",
            job.input.display()
        )
    })
}

fn probe_audio_sample(input: &Path) -> Result<AudioSample> {
    let output = run_ffprobe_candidate(|candidate| {
        Command::new(candidate)
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("a:0")
            .arg("-show_entries")
            .arg("stream=sample_fmt,bits_per_raw_sample,bits_per_sample")
            .arg("-of")
            .arg("default=noprint_wrappers=1")
            .arg(input)
            .stdin(Stdio::null())
            .output()
    })
    .with_context(|| format!("failed to spawn ffprobe for {}", input.display()))?;

    if !output.status.success() {
        bail!(
            "ffprobe exited with status {} for input {}",
            output.status,
            input.display()
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("ffprobe output is not UTF-8 for {}", input.display()))?;
    Ok(parse_audio_sample(&stdout))
}

fn parse_audio_sample(output: &str) -> AudioSample {
    let mut sample_fmt = None;
    let mut raw_bits = None;
    let mut sample_bits = None;

    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();

        match key.trim() {
            "sample_fmt" if !value.is_empty() && value != "N/A" => {
                sample_fmt = Some(value.to_string());
            }
            "bits_per_raw_sample" => raw_bits = parse_bits(value),
            "bits_per_sample" => sample_bits = parse_bits(value),
            _ => {}
        }
    }

    AudioSample {
        sample_fmt,
        bits: raw_bits.or(sample_bits),
    }
}

fn parse_bits(value: &str) -> Option<u16> {
    match value.trim().parse::<u16>() {
        Ok(0) | Err(_) => None,
        Ok(bits) => Some(bits),
    }
}

fn pcm_codec_for(target_format: AudioFormat, sample: &AudioSample) -> Result<&'static str> {
    if let Some(sample_fmt) = sample.sample_fmt.as_deref() {
        match (target_format, sample_fmt) {
            (AudioFormat::Wav, "flt" | "fltp") => return Ok("pcm_f32le"),
            (AudioFormat::Aiff, "flt" | "fltp") => return Ok("pcm_f32be"),
            (AudioFormat::Wav, "dbl" | "dblp") => return Ok("pcm_f64le"),
            (AudioFormat::Aiff, "dbl" | "dblp") => return Ok("pcm_f64be"),
            _ => {}
        }
    }

    let bits = sample
        .bits
        .with_context(|| "ffprobe did not report a usable bit depth")?;
    let bucket = match bits {
        1..=8 => 8,
        9..=16 => 16,
        17..=24 => 24,
        25..=32 => 32,
        _ => bail!("unsupported PCM bit depth: {bits}"),
    };

    match (target_format, bucket) {
        (AudioFormat::Wav, 8) => Ok("pcm_u8"),
        (AudioFormat::Wav, 16) => Ok("pcm_s16le"),
        (AudioFormat::Wav, 24) => Ok("pcm_s24le"),
        (AudioFormat::Wav, 32) => Ok("pcm_s32le"),
        (AudioFormat::Aiff, 8) => Ok("pcm_s8"),
        (AudioFormat::Aiff, 16) => Ok("pcm_s16be"),
        (AudioFormat::Aiff, 24) => Ok("pcm_s24be"),
        (AudioFormat::Aiff, 32) => Ok("pcm_s32be"),
        _ => bail!("PCM codec selection is only supported for WAV and AIFF"),
    }
}

fn ffmpeg_command(
    candidate: &str,
    job: &ConversionJob,
    output: &Path,
    pcm_codec: Option<&'static str>,
) -> Command {
    let mut command = Command::new(candidate);
    command.arg("-nostdin").arg("-i").arg(&job.input);

    match job.target_format {
        AudioFormat::Aiff => {
            command
                .arg("-map")
                .arg("0")
                .arg("-c:a")
                .arg(pcm_codec.expect("AIFF conversion should resolve a PCM codec"))
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
                .arg(pcm_codec.expect("WAV conversion should resolve a PCM codec"));
        }
    }

    command.arg("-y").arg("-loglevel").arg("error").arg(output);
    command
}

#[cfg(test)]
fn command_args(command: &Command) -> Vec<String> {
    use std::ffi::OsString;

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

    use super::{AudioSample, command_args, ffmpeg_command, parse_audio_sample, pcm_codec_for};

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
        let command = ffmpeg_command(
            "ffmpeg",
            &job(AudioFormat::Flac),
            Path::new("out.flac"),
            None,
        );
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "flac"]));
        assert!(!args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.flac"));
    }

    #[test]
    fn builds_aiff_args_with_resolved_pcm_codec() {
        let command = ffmpeg_command(
            "ffmpeg",
            &job(AudioFormat::Aiff),
            Path::new("out.aiff"),
            Some("pcm_s24be"),
        );
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "pcm_s24be"]));
        assert!(args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.aiff"));
    }

    #[test]
    fn builds_wav_args_with_resolved_pcm_codec_and_audio_only_mapping() {
        let command = ffmpeg_command(
            "ffmpeg",
            &job(AudioFormat::Wav),
            Path::new("out.wav"),
            Some("pcm_s32le"),
        );
        let args = command_args(&command);

        assert!(args.windows(2).any(|args| args == ["-map", "0:a:0"]));
        assert!(args.windows(2).any(|args| args == ["-map_metadata", "0"]));
        assert!(args.windows(2).any(|args| args == ["-c:a", "pcm_s32le"]));
        assert!(!args.windows(2).any(|args| args == ["-map", "0"]));
        assert!(!args.windows(2).any(|args| args == ["-write_id3v2", "1"]));
        assert_eq!(args.last().map(String::as_str), Some("out.wav"));
    }

    #[test]
    fn parses_probe_output_using_raw_bits_first() {
        let sample =
            parse_audio_sample("sample_fmt=s32\nbits_per_sample=32\nbits_per_raw_sample=24\n");

        assert_eq!(
            sample,
            AudioSample {
                sample_fmt: Some("s32".to_string()),
                bits: Some(24),
            }
        );
    }

    #[test]
    fn parses_probe_output_ignoring_missing_zero_and_na_bits() {
        let sample =
            parse_audio_sample("sample_fmt=s16\nbits_per_sample=0\nbits_per_raw_sample=N/A\n");

        assert_eq!(
            sample,
            AudioSample {
                sample_fmt: Some("s16".to_string()),
                bits: None,
            }
        );
    }

    #[test]
    fn maps_integer_pcm_without_downconverting() {
        for (format, bits, codec) in [
            (AudioFormat::Wav, 8, "pcm_u8"),
            (AudioFormat::Wav, 16, "pcm_s16le"),
            (AudioFormat::Wav, 20, "pcm_s24le"),
            (AudioFormat::Wav, 32, "pcm_s32le"),
            (AudioFormat::Aiff, 8, "pcm_s8"),
            (AudioFormat::Aiff, 16, "pcm_s16be"),
            (AudioFormat::Aiff, 20, "pcm_s24be"),
            (AudioFormat::Aiff, 32, "pcm_s32be"),
        ] {
            let sample = AudioSample {
                sample_fmt: Some("s32".to_string()),
                bits: Some(bits),
            };

            assert_eq!(pcm_codec_for(format, &sample).unwrap(), codec);
        }
    }

    #[test]
    fn maps_float_pcm_from_sample_format() {
        for (format, sample_fmt, codec) in [
            (AudioFormat::Wav, "flt", "pcm_f32le"),
            (AudioFormat::Wav, "fltp", "pcm_f32le"),
            (AudioFormat::Wav, "dbl", "pcm_f64le"),
            (AudioFormat::Wav, "dblp", "pcm_f64le"),
            (AudioFormat::Aiff, "flt", "pcm_f32be"),
            (AudioFormat::Aiff, "fltp", "pcm_f32be"),
            (AudioFormat::Aiff, "dbl", "pcm_f64be"),
            (AudioFormat::Aiff, "dblp", "pcm_f64be"),
        ] {
            let sample = AudioSample {
                sample_fmt: Some(sample_fmt.to_string()),
                bits: Some(16),
            };

            assert_eq!(pcm_codec_for(format, &sample).unwrap(), codec);
        }
    }

    #[test]
    fn rejects_unknown_pcm_depth() {
        let sample = AudioSample {
            sample_fmt: Some("s64".to_string()),
            bits: Some(64),
        };

        let error = pcm_codec_for(AudioFormat::Wav, &sample).expect_err("64-bit int should fail");
        assert!(error.to_string().contains("unsupported PCM bit depth"));
    }
}
