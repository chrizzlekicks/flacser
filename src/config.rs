use std::{num::NonZeroUsize, path::PathBuf};

use anyhow::Result;

use crate::cli::{Cli, Commands};

#[derive(Debug, Clone)]
pub struct Config {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub dry_run: bool,
    pub jobs: usize,
}

impl Config {
    pub fn from_cli(cli: Cli) -> Self {
        let Commands::Convert(convert) = cli.command;

        Self {
            input_path: convert.input_path,
            output_dir: convert.output_dir,
            dry_run: convert.dry_run,
            jobs: default_jobs(),
        }
    }
}

pub fn resolve(cli: Cli) -> Result<Config> {
    Ok(Config::from_cli(cli))
}

fn default_jobs() -> usize {
    let cpus = std::thread::available_parallelism()
        .unwrap_or(NonZeroUsize::new(1).expect("1 is non-zero"))
        .get();

    cpus.saturating_sub(1).max(1)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::cli::{Cli, Commands, ConvertArgs};

    use super::Config;

    #[test]
    fn from_cli_maps_convert_args() {
        let cli = Cli {
            command: Commands::Convert(ConvertArgs {
                input_path: PathBuf::from("in.flac"),
                output_dir: Some(PathBuf::from("out")),
                dry_run: true,
            }),
        };

        let config = Config::from_cli(cli);

        assert_eq!(config.input_path, PathBuf::from("in.flac"));
        assert_eq!(config.output_dir, Some(PathBuf::from("out")));
        assert!(config.dry_run);
    }

    #[test]
    fn default_jobs_is_always_at_least_one() {
        let cli = Cli {
            command: Commands::Convert(ConvertArgs {
                input_path: PathBuf::from("in.flac"),
                output_dir: None,
                dry_run: false,
            }),
        };

        let config = Config::from_cli(cli);
        assert!(config.jobs >= 1);
    }
}
