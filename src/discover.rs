use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::config::Config;

pub fn discover(config: &Config) -> Result<Vec<PathBuf>> {
    let input_path = &config.input_path.as_path();

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

    let max_depth = if config.recursive { usize::MAX } else { 1 };
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::config::Config;

    use super::discover;

    fn test_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "flacser-discover-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn test_config(input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: None,
            overwrite: false,
            dry_run: false,
            recursive: false,
            jobs: 1,
        }
    }

    fn recursive_test_config(input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: None,
            overwrite: false,
            dry_run: false,
            recursive: true,
            jobs: 1,
        }
    }

    #[test]
    fn discovers_single_flac_input_file() {
        let dir = test_dir("single");
        let input = dir.join("song.flac");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![input]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_non_flac_input_file() {
        let dir = test_dir("reject-non-flac");
        let input = dir.join("song.wav");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone());
        let error = discover(&config).expect_err("discover should fail");
        assert!(error.to_string().contains("input file is not a .flac file"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn discovers_directory_non_recursive_and_sorted() {
        let dir = test_dir("dir");
        let a = dir.join("a.flac");
        let z = dir.join("z.flac");
        let txt = dir.join("ignore.txt");
        let nested_dir = dir.join("nested");
        let nested = nested_dir.join("nested.flac");

        fs::write(&z, b"").expect("create z");
        fs::write(&a, b"").expect("create a");
        fs::write(&txt, b"").expect("create txt");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested, b"").expect("create nested");

        let config = test_config(dir.clone());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![a, z]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn discovers_directory_recursively_when_enabled() {
        let dir = test_dir("recursive-dir");
        let a = dir.join("a.flac");
        let nested_dir = dir.join("nested");
        let nested = nested_dir.join("nested.flac");

        fs::write(&a, b"").expect("create a");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested, b"").expect("create nested");

        let config = recursive_test_config(dir.clone());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![a, nested]);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn errors_when_input_path_does_not_exist() {
        let missing = test_dir("missing-base").join("does-not-exist");
        let config = test_config(missing.clone());

        let error = discover(&config).expect_err("discover should fail");
        assert!(
            error
                .to_string()
                .contains("input path does not exist or is not accessible")
        );
    }
}
