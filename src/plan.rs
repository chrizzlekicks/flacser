use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub input: PathBuf,
    pub output: PathBuf,
}

pub fn plan(config: &Config, inputs: Vec<PathBuf>) -> Result<Vec<ConversionJob>> {
    let original_input = config.input_path.as_path();
    let output_dir = config.output_dir.as_deref();
    validate_output_dir(output_dir)?;

    let input_is_directory = original_input.is_dir();
    let jobs = inputs
        .into_iter()
        .map(|input| {
            let output = if input_is_directory {
                let relative = input.strip_prefix(original_input).with_context(|| {
                    format!(
                        "could not derive relative path from {} against {}",
                        input.display(),
                        original_input.display()
                    )
                })?;

                let root = output_dir.unwrap_or(original_input);
                let mut output = root.join(relative);
                output.set_extension("aiff");
                output
            } else {
                let file_name = input
                    .file_name()
                    .with_context(|| format!("input file has no file name: {}", input.display()))?;

                let mut output_file_name = PathBuf::from(file_name);
                output_file_name.set_extension("aiff");

                let root = output_dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")));
                root.join(output_file_name)
            };

            Ok(ConversionJob { input, output })
        })
        .collect::<Result<Vec<_>>>()?;

    detect_output_collisions(&jobs)?;
    Ok(jobs)
}

fn validate_output_dir(output_dir: Option<&Path>) -> Result<()> {
    match output_dir {
        None => Ok(()),
        Some(dir) => {
            if dir.exists() && !dir.is_dir() {
                bail!(
                    "output path exists but is not a directory: {}",
                    dir.display()
                );
            }

            Ok(())
        }
    }
}

fn detect_output_collisions(jobs: &Vec<ConversionJob>) -> Result<()> {
    let mut seen: HashMap<&Path, &Path> = HashMap::new();

    for job in jobs {
        match seen.get(job.output.as_path()) {
            Some(existing_input) => {
                bail!(
                    "output collision detected: {} and {} both map to {}",
                    existing_input.display(),
                    job.input.display(),
                    job.output.display()
                );
            }
            None => {}
        }
        seen.insert(job.output.as_path(), job.input.as_path());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::config::Config;

    use super::plan;

    fn test_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("flacser-plan-{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn test_config(input_path: PathBuf, output_dir: Option<PathBuf>) -> Config {
        Config {
            input_path,
            output_dir,
            overwrite: false,
            dry_run: false,
            recursive: false,
            jobs: 1,
        }
    }

    #[test]
    fn single_file_maps_to_same_parent_by_default() {
        let dir = test_dir("single-default-output");
        let input = dir.join("track.flac");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone(), None);
        let jobs = plan(&config, vec![input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, input);
        assert_eq!(jobs[0].output, dir.join("track.aiff"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn single_file_maps_to_output_dir_when_provided() {
        let dir = test_dir("single-custom-output");
        let input = dir.join("track.flac");
        let output_dir = dir.join("out");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone(), Some(output_dir.clone()));
        let jobs = plan(&config, vec![input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, input);
        assert_eq!(jobs[0].output, output_dir.join("track.aiff"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn directory_mode_preserves_relative_structure() {
        let dir = test_dir("directory-relative");
        let input_root = dir.join("input");
        let nested_dir = input_root.join("album");
        let nested_input = nested_dir.join("song.flac");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested_input, b"").expect("create nested input");

        let config = test_config(input_root.clone(), None);
        let jobs = plan(&config, vec![nested_input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, nested_input);
        assert_eq!(jobs[0].output, input_root.join("album/song.aiff"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_output_dir_when_path_is_file() {
        let dir = test_dir("output-file");
        let input = dir.join("track.flac");
        let output_path = dir.join("out");
        fs::write(&input, b"").expect("create input");
        fs::write(&output_path, b"").expect("create output file");

        let config = test_config(input.clone(), Some(output_path.clone()));
        let error = plan(&config, vec![input]).expect_err("plan should fail");
        assert!(
            error
                .to_string()
                .contains("output path exists but is not a directory")
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_output_collisions_before_execution() {
        let dir = test_dir("collision");
        let output_dir = dir.join("out");
        fs::create_dir_all(&output_dir).expect("create output dir");

        let original_input = dir.join("original.flac");
        fs::write(&original_input, b"").expect("create original input");

        let input_a_dir = dir.join("a");
        let input_b_dir = dir.join("b");
        fs::create_dir_all(&input_a_dir).expect("create a dir");
        fs::create_dir_all(&input_b_dir).expect("create b dir");
        let input_a = input_a_dir.join("song.flac");
        let input_b = input_b_dir.join("song.flac");
        fs::write(&input_a, b"").expect("create input a");
        fs::write(&input_b, b"").expect("create input b");

        let config = test_config(original_input, Some(output_dir));
        let error = plan(&config, vec![input_a, input_b]).expect_err("plan should fail");
        assert!(error.to_string().contains("output collision detected"));

        let _ = fs::remove_dir_all(dir);
    }
}
