use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::config::Config;

pub fn discover(config: &Config) -> Result<Vec<PathBuf>> {
    let input_path: &Path = &config.input_path;

    if input_path.is_file() {
        if !is_flac(input_path) {
            bail!("input file is not a .flac file: {}", input_path.display());
        }
        return Ok(vec![input_path.to_path_buf()]);
    }

    if !input_path.is_dir() {
        bail!(
            "input path does not exist or is not accessible: {}",
            input_path.display()
        );
    }

    let max_depth = 1;
    let mut files = Vec::new();

    for entry in WalkDir::new(input_path).max_depth(max_depth) {
        let entry = entry.context("failed while scanning input directory")?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }
        if is_flac(path) {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}

fn is_flac(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
}
