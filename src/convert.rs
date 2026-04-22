use std::fs;
use std::path::PathBuf;

use rayon::prelude::*;

use crate::config::Config;
use crate::ffmpeg::run_ffmpeg;
use crate::plan::ConversionJob;

#[derive(Debug, Clone)]
pub enum JobResult {
    Converted,
    Skipped,
    Failed { input: PathBuf, error: String },
}

pub fn execute(config: &Config, jobs: Vec<ConversionJob>) -> Vec<JobResult> {
    let configured_jobs = config.jobs;
    let dry_run = config.dry_run;

    if jobs.is_empty() {
        return Vec::new();
    }

    let workers = configured_jobs.min(jobs.len()).max(1);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .expect("failed to build rayon threadpool");

    pool.install(|| {
        jobs.into_par_iter()
            .map(|job| execute_job(job, dry_run))
            .collect()
    })
}

fn execute_job(job: ConversionJob, dry_run: bool) -> JobResult {
    if job.output.exists() {
        return JobResult::Skipped;
    }

    if dry_run {
        return JobResult::Converted;
    }

    if let Some(parent_dir) = job.output.parent()
        && let Err(error) = fs::create_dir_all(parent_dir)
    {
        return JobResult::Failed {
            input: job.input,
            error: format!(
                "failed to create output directory {}: {error}",
                parent_dir.display()
            ),
        };
    }

    match run_ffmpeg(&job.input, &job.output) {
        Ok(()) => JobResult::Converted,
        Err(error) => JobResult::Failed {
            input: job.input.clone(),
            error: format!(
                "{} -> {}: {error:#}",
                job.input.display(),
                job.output.display()
            ),
        },
    }
}
