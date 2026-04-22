use std::{num::NonZeroUsize, path::PathBuf};

use anyhow::Result;

use crate::cli::Cli;

#[derive(Debug, Clone)]
pub struct Config {
    pub input_path: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub dry_run: bool,
    pub jobs: usize,
}

impl Config {
    pub fn from_cli(cli: Cli) -> Self {
        Self {
            input_path: cli.input_path,
            output_dir: cli.output_dir,
            dry_run: cli.dry_run,
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
