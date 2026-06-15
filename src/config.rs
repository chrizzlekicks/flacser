use std::{env, num::NonZeroUsize, path::PathBuf, thread};

use anyhow::{Context, Result, bail};

use crate::audio_format::AudioFormat;
use crate::cli::ConvertArgs;

const CONVERT_TO_ENV: &str = "FLACSER_CONVERT_TO";

#[derive(Debug, Clone)]
pub struct Config {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub overwrite: bool,
    pub dry_run: bool,
    pub recursive: bool,
    pub jobs: usize,
    pub target_format: AudioFormat,
}

impl Config {
    pub fn from_convert_args(convert: ConvertArgs) -> Result<Self> {
        let target_format = resolve_target_format(convert.to)?;

        Ok(Self {
            input_path: convert.input_path,
            output_dir: convert.output_dir,
            overwrite: convert.overwrite,
            dry_run: convert.dry_run,
            recursive: convert.recursive,
            jobs: convert
                .jobs
                .map(NonZeroUsize::get)
                .unwrap_or_else(default_jobs),
            target_format,
        })
    }
}

fn resolve_target_format(cli_target: Option<AudioFormat>) -> Result<AudioFormat> {
    if let Some(target) = cli_target {
        return Ok(target);
    }

    match env::var(CONVERT_TO_ENV) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("invalid {CONVERT_TO_ENV} value")),
        Err(env::VarError::NotPresent) => {
            bail!("target format is required; pass --to <format> or set {CONVERT_TO_ENV}")
        }
        Err(env::VarError::NotUnicode(_)) => bail!("{CONVERT_TO_ENV} is not valid Unicode"),
    }
}

pub fn detected_cpu_cores() -> usize {
    thread::available_parallelism()
        .unwrap_or(NonZeroUsize::new(1).expect("1 is non-zero"))
        .get()
}

pub fn default_jobs() -> usize {
    default_jobs_for_cpu_count(detected_cpu_cores())
}

pub fn default_jobs_for_cpu_count(cpus: usize) -> usize {
    cpus.saturating_sub(1).max(1)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    use crate::cli::ConvertArgs;

    use super::Config;

    #[test]
    fn from_convert_args_maps_convert_args() {
        let args = ConvertArgs {
            input_path: PathBuf::from("in.flac"),
            to: Some(crate::audio_format::AudioFormat::Aiff),
            output_dir: Some(PathBuf::from("out")),
            overwrite: true,
            dry_run: true,
            recursive: true,
            jobs: NonZeroUsize::new(2),
        };

        let config = Config::from_convert_args(args).expect("config should resolve");

        assert_eq!(config.input_path, PathBuf::from("in.flac"));
        assert_eq!(config.output_dir, Some(PathBuf::from("out")));
        assert!(config.overwrite);
        assert!(config.dry_run);
        assert!(config.recursive);
        assert_eq!(config.jobs, 2);
        assert_eq!(config.target_format, crate::audio_format::AudioFormat::Aiff);
    }

    #[test]
    fn default_jobs_is_always_at_least_one() {
        let args = ConvertArgs {
            input_path: PathBuf::from("in.flac"),
            to: Some(crate::audio_format::AudioFormat::Aiff),
            output_dir: None,
            overwrite: false,
            dry_run: false,
            recursive: false,
            jobs: None,
        };

        let config = Config::from_convert_args(args).expect("config should resolve");
        assert!(config.jobs >= 1);
    }

    #[test]
    fn default_jobs_leaves_one_core_free_when_possible() {
        assert_eq!(super::default_jobs_for_cpu_count(1), 1);
        assert_eq!(super::default_jobs_for_cpu_count(2), 1);
        assert_eq!(super::default_jobs_for_cpu_count(8), 7);
    }
}
