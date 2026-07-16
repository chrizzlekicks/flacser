use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use walkdir::WalkDir;

use crate::audio_format::AudioFormat;
use crate::config::Config;

pub fn discover(config: &Config) -> Result<Vec<PathBuf>> {
    discover_with_excluded_target(
        config.input_path.as_path(),
        config.recursive,
        Some(config.target_format),
    )
}

pub fn discover_for_doctor(input_path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    discover_with_excluded_target(input_path, recursive, None)
}

fn discover_with_excluded_target(
    input_path: &Path,
    recursive: bool,
    excluded_target: Option<AudioFormat>,
) -> Result<Vec<PathBuf>> {
    if input_path.is_file() {
        if AudioFormat::from_path(input_path).is_none() {
            bail!(
                "input file is not a supported audio file (.flac, .aiff, .aif, .wav): {}",
                input_path.display()
            );
        }
        return Ok(vec![input_path.to_path_buf()]);
    }

    if !input_path.is_dir() {
        bail!(
            "input path does not exist or is not accessible: {}",
            input_path.display()
        );
    }

    let max_depth = if recursive { usize::MAX } else { 1 };
    let mut files = Vec::new();

    for entry in WalkDir::new(input_path).max_depth(max_depth) {
        let entry = entry.context("failed while scanning input directory")?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }
        if AudioFormat::from_path(path).is_some_and(|format| Some(format) != excluded_target) {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::config::Config;

    use super::{discover, discover_for_doctor};

    fn test_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create test dir")
    }

    fn test_config(input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: None,
            dry_run: false,
            recursive: false,
            flatten: false,
            jobs: 1,
            target_format: crate::audio_format::AudioFormat::Aiff,
        }
    }

    fn recursive_test_config(input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: None,
            dry_run: false,
            recursive: true,
            flatten: false,
            jobs: 1,
            target_format: crate::audio_format::AudioFormat::Aiff,
        }
    }

    #[test]
    fn discovers_single_flac_input_file() {
        let dir = test_dir();
        let input = dir.path().join("song.flac");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![input]);
    }

    #[test]
    fn rejects_unsupported_input_file() {
        let dir = test_dir();
        let input = dir.path().join("song.mp3");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone());
        let error = discover(&config).expect_err("discover should fail");
        assert!(
            error
                .to_string()
                .contains("input file is not a supported audio file")
        );
    }

    #[test]
    fn discovers_directory_non_recursive_and_sorted() {
        let dir = test_dir();
        let a = dir.path().join("a.flac");
        let z = dir.path().join("z.flac");
        let txt = dir.path().join("ignore.txt");
        let nested_dir = dir.path().join("nested");
        let nested = nested_dir.join("nested.flac");

        fs::write(&z, b"").expect("create z");
        fs::write(&a, b"").expect("create a");
        fs::write(&txt, b"").expect("create txt");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested, b"").expect("create nested");

        let config = test_config(dir.path().to_path_buf());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![a, z]);
    }

    #[test]
    fn discovers_supported_input_files() {
        let dir = test_dir();
        let flac = dir.path().join("song.flac");
        let aiff = dir.path().join("song.aiff");
        let aif = dir.path().join("song.aif");
        let wav = dir.path().join("song.wav");
        let txt = dir.path().join("ignore.txt");

        fs::write(&flac, b"").expect("create flac");
        fs::write(&aiff, b"").expect("create aiff");
        fs::write(&aif, b"").expect("create aif");
        fs::write(&wav, b"").expect("create wav");
        fs::write(&txt, b"").expect("create txt");

        let files = discover_for_doctor(dir.path(), false).expect("discover should succeed");
        assert_eq!(files, vec![aif, aiff, flac, wav]);
    }

    #[test]
    fn discovers_directory_recursively_when_enabled() {
        let dir = test_dir();
        let a = dir.path().join("a.flac");
        let nested_dir = dir.path().join("nested");
        let nested = nested_dir.join("nested.flac");

        fs::write(&a, b"").expect("create a");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested, b"").expect("create nested");

        let config = recursive_test_config(dir.path().to_path_buf());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![a, nested]);
    }

    #[test]
    fn excludes_target_format_files_when_discovering_directory_inputs() {
        let dir = test_dir();
        let flac = dir.path().join("song.flac");
        let aiff = dir.path().join("song.aiff");

        fs::write(&flac, b"").expect("create flac");
        fs::write(&aiff, b"").expect("create aiff");

        let config = test_config(dir.path().to_path_buf());
        let files = discover(&config).expect("discover should succeed");
        assert_eq!(files, vec![flac]);
    }

    #[test]
    fn errors_when_input_path_does_not_exist() {
        let missing = test_dir().path().join("does-not-exist");
        let config = test_config(missing.clone());

        let error = discover(&config).expect_err("discover should fail");
        assert!(
            error
                .to_string()
                .contains("input path does not exist or is not accessible")
        );
    }
}
