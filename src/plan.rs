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
    let original_input: &Path = &config.input_path;
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
    if let Some(dir) = output_dir
        && dir.exists()
        && !dir.is_dir()
    {
        bail!(
            "output path exists but is not a directory: {}",
            dir.display()
        );
    }

    Ok(())
}

fn detect_output_collisions(jobs: &[ConversionJob]) -> Result<()> {
    let mut seen: HashMap<&Path, &Path> = HashMap::new();

    for job in jobs {
        if let Some(existing_input) = seen.get(job.output.as_path()) {
            bail!(
                "output collision detected: {} and {} both map to {}",
                existing_input.display(),
                job.input.display(),
                job.output.display()
            );
        }
        seen.insert(job.output.as_path(), job.input.as_path());
    }

    Ok(())
}
