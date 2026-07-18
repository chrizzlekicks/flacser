use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::audio_format::AudioFormat;
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct ConversionJob {
    pub input: PathBuf,
    pub output: PathBuf,
    pub source_format: AudioFormat,
    pub target_format: AudioFormat,
}

pub fn plan(config: &Config, inputs: Vec<PathBuf>) -> Result<Vec<ConversionJob>> {
    let original_input_path = config.input_path.as_path();
    let output_dir = config.output_dir.as_deref();
    let target_format = config.target_format;
    let flatten = config.flatten;
    validate_output_dir(output_dir)?;

    let jobs = inputs
        .into_iter()
        .map(|input| {
            let source_format = AudioFormat::from_path(&input)
                .with_context(|| format!("unsupported input file: {}", input.display()))?;

            if source_format == target_format {
                bail!(
                    "source and target formats are both {target_format}; same-format conversion is not supported for {}",
                    input.display()
                );
            }

            let output =
                planned_output_path(original_input_path, &input, output_dir, target_format, flatten)?;

            Ok(ConversionJob {
                input,
                output,
                source_format,
                target_format,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    detect_output_collisions(&jobs, config.flatten)?;

    Ok(jobs)
}

pub(crate) fn validate_output_dir(output_dir: Option<&Path>) -> Result<()> {
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

pub(crate) fn planned_output_path(
    original_input: &Path,
    input: &Path,
    output_dir: Option<&Path>,
    target_format: AudioFormat,
    flatten: bool,
) -> Result<PathBuf> {
    let input_is_directory = original_input.is_dir();

    if input_is_directory {
        let root = output_dir.unwrap_or(original_input);
        let mut output = if flatten {
            let file_name = input
                .file_name()
                .with_context(|| format!("input file has no file name: {}", input.display()))?;

            root.join(file_name)
        } else {
            let relative = input.strip_prefix(original_input).with_context(|| {
                format!(
                    "could not derive relative path from {} against {}",
                    input.display(),
                    original_input.display()
                )
            })?;
            root.join(relative)
        };

        output.set_extension(target_format.as_str());
        Ok(output)
    } else {
        let file_name = input
            .file_name()
            .with_context(|| format!("input file has no file name: {}", input.display()))?;

        let mut output_file_name = PathBuf::from(file_name);
        output_file_name.set_extension(target_format.as_str());

        let root = output_dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")));
        Ok(root.join(output_file_name))
    }
}

pub(crate) fn detect_output_collisions(jobs: &[ConversionJob], flatten: bool) -> Result<()> {
    if flatten {
        let mut seen: HashMap<Vec<u8>, &Path> = HashMap::new();

        for job in jobs {
            let mut collision_key = output_file_name_bytes(job.output.as_path())?;
            collision_key.make_ascii_lowercase();
            #[expect(clippy::single_match, reason = "match pattern keeps bailing clear")]
            match seen.get(&collision_key) {
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
            seen.insert(collision_key, job.input.as_path());
        }
    } else {
        let mut seen: HashMap<&Path, &Path> = HashMap::new();

        for job in jobs {
            if let Some(existing_input) = seen.get(job.output.as_path()) {
                bail!(
                    "output collision detected: {} and {} both map to {}",
                    existing_input.display(),
                    job.input.display(),
                    job.output.display()
                )
            }
            seen.insert(job.output.as_path(), job.input.as_path());
        }
    }

    Ok(())
}

fn output_file_name_bytes(output: &Path) -> Result<Vec<u8>> {
    let file_name = output
        .file_name()
        .with_context(|| format!("output path has no file name: {}", output.display()))?;
    Ok(file_name.as_encoded_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, path::PathBuf};

    #[cfg(target_os = "linux")]
    use std::os::unix::ffi::OsStringExt;

    use crate::{audio_format::AudioFormat, config::Config};

    use super::plan;

    fn test_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("create test dir")
    }

    fn test_config_to(
        input_path: PathBuf,
        output_dir: Option<PathBuf>,
        target_format: AudioFormat,
    ) -> Config {
        Config {
            input_path,
            output_dir,
            dry_run: false,
            recursive: false,
            flatten: false,
            jobs: 1,
            target_format,
        }
    }

    fn test_config(input_path: PathBuf, output_dir: Option<PathBuf>) -> Config {
        test_config_to(input_path, output_dir, AudioFormat::Aiff)
    }

    #[test]
    fn single_file_maps_to_same_parent_by_default() {
        let dir = test_dir();
        let input = dir.path().join("track.flac");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone(), None);
        let jobs = plan(&config, vec![input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, input);
        assert_eq!(jobs[0].output, dir.path().join("track.aiff"));
        assert_eq!(jobs[0].source_format, AudioFormat::Flac);
        assert_eq!(jobs[0].target_format, AudioFormat::Aiff);
    }

    #[test]
    fn cross_format_routes_map_to_target_extension() {
        for (input_name, target, output_name, source) in [
            (
                "track.flac",
                AudioFormat::Wav,
                "track.wav",
                AudioFormat::Flac,
            ),
            (
                "track.wav",
                AudioFormat::Flac,
                "track.flac",
                AudioFormat::Wav,
            ),
            (
                "track.aiff",
                AudioFormat::Flac,
                "track.flac",
                AudioFormat::Aiff,
            ),
            (
                "track.aif",
                AudioFormat::Flac,
                "track.flac",
                AudioFormat::Aiff,
            ),
            (
                "track.aiff",
                AudioFormat::Wav,
                "track.wav",
                AudioFormat::Aiff,
            ),
            (
                "track.wav",
                AudioFormat::Aiff,
                "track.aiff",
                AudioFormat::Wav,
            ),
        ] {
            let dir = test_dir();
            let input = dir.path().join(input_name);
            fs::write(&input, b"").expect("create input");

            let config = test_config_to(input.clone(), None, target);
            let jobs = plan(&config, vec![input]).expect("plan should succeed");

            assert_eq!(jobs[0].output, dir.path().join(output_name));
            assert_eq!(jobs[0].source_format, source);
            assert_eq!(jobs[0].target_format, target);
        }
    }

    #[test]
    fn rejects_same_format_conversion() {
        let dir = test_dir();
        let input = dir.path().join("track.wav");
        fs::write(&input, b"").expect("create input");

        let config = test_config_to(input.clone(), None, AudioFormat::Wav);
        let error = plan(&config, vec![input]).expect_err("plan should fail");
        assert!(error.to_string().contains("same-format conversion"));
    }

    #[test]
    fn single_file_maps_to_output_dir_when_provided() {
        let dir = test_dir();
        let input = dir.path().join("track.flac");
        let output_dir = dir.path().join("out");
        fs::write(&input, b"").expect("create input");

        let config = test_config(input.clone(), Some(output_dir.clone()));
        let jobs = plan(&config, vec![input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, input);
        assert_eq!(jobs[0].output, output_dir.join("track.aiff"));
    }

    #[test]
    fn directory_mode_preserves_relative_structure() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let nested_dir = input_root.join("album");
        let nested_input = nested_dir.join("song.flac");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        fs::write(&nested_input, b"").expect("create nested input");

        let config = test_config(input_root.clone(), None);
        let jobs = plan(&config, vec![nested_input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].input, nested_input);
        assert_eq!(jobs[0].output, input_root.join("album/song.aiff"));
    }

    #[test]
    fn rejects_output_dir_when_path_is_file() {
        let dir = test_dir();
        let input = dir.path().join("track.flac");
        let output_path = dir.path().join("out");
        fs::write(&input, b"").expect("create input");
        fs::write(&output_path, b"").expect("create output file");

        let config = test_config(input.clone(), Some(output_path.clone()));
        let error = plan(&config, vec![input]).expect_err("plan should fail");
        assert!(
            error
                .to_string()
                .contains("output path exists but is not a directory")
        );
    }

    #[test]
    fn detects_output_collisions_before_execution() {
        let dir = test_dir();
        let output_dir = dir.path().join("out");
        fs::create_dir_all(&output_dir).expect("create output dir");

        let original_input = dir.path().join("original.flac");
        fs::write(&original_input, b"").expect("create original input");

        let input_a_dir = dir.path().join("a");
        let input_b_dir = dir.path().join("b");
        fs::create_dir_all(&input_a_dir).expect("create a dir");
        fs::create_dir_all(&input_b_dir).expect("create b dir");
        let input_a = input_a_dir.join("song.flac");
        let input_b = input_b_dir.join("song.flac");
        fs::write(&input_a, b"").expect("create input a");
        fs::write(&input_b, b"").expect("create input b");

        let config = test_config(original_input, Some(output_dir));
        let error = plan(&config, vec![input_a, input_b]).expect_err("plan should fail");
        assert!(error.to_string().contains("output collision detected"));
    }

    #[test]
    fn flatten_directory_maps_to_input_root() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let album_a = input_root.join("album-a");
        let album_b = input_root.join("album-b");
        fs::create_dir_all(&album_a).expect("create album-a");
        fs::create_dir_all(&album_b).expect("create album-b");
        let song_a = album_a.join("song.flac");
        let track_b = album_b.join("track.flac");
        fs::write(&song_a, b"").expect("create song a");
        fs::write(&track_b, b"").expect("create track b");

        let mut config = test_config(input_root.clone(), None);
        config.flatten = true;
        let jobs =
            plan(&config, vec![song_a.clone(), track_b.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].output, input_root.join("song.aiff"));
        assert_eq!(jobs[1].output, input_root.join("track.aiff"));
    }

    #[test]
    fn flatten_directory_with_output_dir_maps_to_output_root() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let album_a = input_root.join("album-a");
        fs::create_dir_all(&album_a).expect("create album-a");
        let song_a = album_a.join("song.flac");
        fs::write(&song_a, b"").expect("create song a");

        let output_dir = dir.path().join("output");
        fs::create_dir_all(&output_dir).expect("create output dir");

        let mut config = test_config(input_root.clone(), Some(output_dir.clone()));
        config.flatten = true;
        let jobs = plan(&config, vec![song_a.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].output, output_dir.join("song.aiff"));
    }

    #[test]
    fn flatten_detects_collisions_from_different_subdirs() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let album_a = input_root.join("album-a");
        let album_b = input_root.join("album-b");
        fs::create_dir_all(&album_a).expect("create album-a");
        fs::create_dir_all(&album_b).expect("create album-b");
        let song_a = album_a.join("song.flac");
        let song_b = album_b.join("song.flac");
        fs::write(&song_a, b"").expect("create song a");
        fs::write(&song_b, b"").expect("create song b");

        let mut config = test_config(input_root.clone(), None);
        config.flatten = true;
        let error = plan(&config, vec![song_a, song_b]).expect_err("plan should fail");
        assert!(error.to_string().contains("output collision detected"));
    }

    #[test]
    fn flatten_detects_case_insensitive_collisions() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let album_a = input_root.join("album-a");
        let album_b = input_root.join("album-b");
        fs::create_dir_all(&album_a).expect("create album-a");
        fs::create_dir_all(&album_b).expect("create album-b");
        let song_a = album_a.join("Song.flac");
        let song_b = album_b.join("song.flac");
        fs::write(&song_a, b"").expect("create song a");
        fs::write(&song_b, b"").expect("create song b");

        let mut config = test_config(input_root.clone(), None);
        config.flatten = true;
        let error = plan(&config, vec![song_a, song_b]).expect_err("plan should fail");
        assert!(error.to_string().contains("output collision detected"));
    }

    #[test]
    fn flatten_is_noop_for_single_file() {
        let dir = test_dir();
        let input = dir.path().join("track.flac");
        fs::write(&input, b"").expect("create input");

        let output_dir = dir.path().join("output");
        fs::create_dir_all(&output_dir).expect("create output dir");

        let mut config = test_config(input.clone(), Some(output_dir.clone()));
        config.flatten = true;
        let jobs = plan(&config, vec![input.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].output, output_dir.join("track.aiff"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_flatten_allows_distinct_non_utf8_paths() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        fs::create_dir_all(&input_root).expect("create input root");

        let input_a = input_root.join(OsString::from_vec(b"a\xff.flac".to_vec()));
        let input_b = input_root.join(OsString::from_vec(b"a\xfe.flac".to_vec()));
        fs::write(&input_a, b"").expect("create input a");
        fs::write(&input_b, b"").expect("create input b");

        let config = test_config(input_root.clone(), None);
        let jobs =
            plan(&config, vec![input_a.clone(), input_b.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 2);
        assert_ne!(jobs[0].output, jobs[1].output);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn flatten_allows_distinct_non_utf8_paths() {
        let dir = test_dir();
        let input_root = dir.path().join("input");
        let album_a = input_root.join("album-a");
        let album_b = input_root.join("album-b");
        fs::create_dir_all(&album_a).expect("create album-a");
        fs::create_dir_all(&album_b).expect("create album-b");

        let input_a = album_a.join(OsString::from_vec(b"a\xff.flac".to_vec()));
        let input_b = album_b.join(OsString::from_vec(b"a\xfe.flac".to_vec()));
        fs::write(&input_a, b"").expect("create input a");
        fs::write(&input_b, b"").expect("create input b");

        let mut config = test_config(input_root.clone(), None);
        config.flatten = true;
        let jobs =
            plan(&config, vec![input_a.clone(), input_b.clone()]).expect("plan should succeed");
        assert_eq!(jobs.len(), 2);
        assert_ne!(jobs[0].output, jobs[1].output);
    }
}
