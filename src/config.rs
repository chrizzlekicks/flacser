use std::{num::NonZeroUsize, path::PathBuf, thread};

use crate::cli::ConvertArgs;

#[derive(Debug, Clone)]
pub struct Config {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub dry_run: bool,
    pub recursive: bool,
    pub flatten: bool,
    pub jobs: usize,
}

impl Config {
    pub fn from_convert_args(convert: ConvertArgs) -> Self {
        Self {
            input_path: convert.input_path,
            output_dir: convert.output_dir,
            dry_run: convert.dry_run,
            recursive: convert.recursive,
            flatten: convert.flatten,
            jobs: convert
                .jobs
                .map(NonZeroUsize::get)
                .unwrap_or_else(default_jobs),
        }
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
            output_dir: Some(PathBuf::from("out")),
            dry_run: true,
            recursive: true,
            flatten: true,
            jobs: NonZeroUsize::new(2),
        };

        let config = Config::from_convert_args(args);

        assert_eq!(config.input_path, PathBuf::from("in.flac"));
        assert_eq!(config.output_dir, Some(PathBuf::from("out")));
        assert!(config.dry_run);
        assert!(config.recursive);
        assert!(config.flatten);
        assert_eq!(config.jobs, 2);
    }

    #[test]
    fn default_jobs_is_always_at_least_one() {
        let args = ConvertArgs {
            input_path: PathBuf::from("in.flac"),
            output_dir: None,
            dry_run: false,
            recursive: false,
            flatten: false,
            jobs: None,
        };

        let config = Config::from_convert_args(args);
        assert!(config.jobs >= 1);
    }

    #[test]
    fn default_jobs_leaves_one_core_free_when_possible() {
        assert_eq!(super::default_jobs_for_cpu_count(1), 1);
        assert_eq!(super::default_jobs_for_cpu_count(2), 1);
        assert_eq!(super::default_jobs_for_cpu_count(8), 7);
    }
}
