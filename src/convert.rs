use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;

use rayon::prelude::*;

use crate::config::Config;
use crate::ffmpeg::run_ffmpeg;
use crate::plan::ConversionJob;

type FfmpegRunner = fn(&Path, &Path) -> Result<()>;

#[derive(Debug, Clone)]
pub enum JobResult {
    Converted,
    Skipped,
    Failed { input: PathBuf, error: String },
}

#[derive(Debug)]
pub struct ProgressReporter {
    total: usize,
    done: Mutex<usize>,
}

impl ProgressReporter {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            done: Mutex::new(0),
        }
    }

    pub fn job_done(&self) {
        let mut completed = self.done.lock().expect("progress reporter mutex poisoned");
        *completed += 1;
        println!("[{}/{}] done", *completed, self.total)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub results: Vec<JobResult>,
    pub workers: usize,
}

pub fn execute(config: &Config, jobs: Vec<ConversionJob>) -> ExecutionReport {
    execute_with_runner(config, jobs, run_ffmpeg)
}

fn execute_with_runner(
    config: &Config,
    jobs: Vec<ConversionJob>,
    runner: FfmpegRunner,
) -> ExecutionReport {
    let configured_jobs = config.jobs;
    let overwrite = config.overwrite;
    let dry_run = config.dry_run;

    if jobs.is_empty() {
        return ExecutionReport {
            results: Vec::new(),
            workers: 0,
        };
    }

    let reporter = ProgressReporter::new(jobs.len());

    let workers = configured_jobs.min(jobs.len()).max(1);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .expect("failed to build rayon threadpool");

    let results = pool.install(|| {
        jobs.into_par_iter()
            .map(|job| {
                let result = execute_job(job, overwrite, dry_run, runner);
                reporter.job_done();
                result
            })
            .collect()
    });

    ExecutionReport { results, workers }
}

fn execute_job(
    job: ConversionJob,
    overwrite: bool,
    dry_run: bool,
    runner: FfmpegRunner,
) -> JobResult {
    if job.output.exists() && !overwrite {
        return JobResult::Skipped;
    }

    if dry_run {
        return JobResult::Converted;
    }

    if let Some(parent_dir) = job.output.parent() {
        if let Err(error) = fs::create_dir_all(parent_dir) {
            return JobResult::Failed {
                input: job.input,
                error: format!(
                    "failed to create output directory {}: {error}",
                    parent_dir.display()
                ),
            };
        }
    }

    match runner(&job.input, &job.output) {
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::{Result, anyhow};

    use crate::{config::Config, plan::ConversionJob};

    use super::{JobResult, execute_with_runner};

    fn test_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "flacser-convert-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn test_config(dry_run: bool, overwrite: bool) -> Config {
        Config {
            input_path: PathBuf::from("ignored"),
            output_dir: None,
            overwrite,
            dry_run,
            recursive: false,
            jobs: 1,
        }
    }

    fn runner_ok(_: &Path, _: &Path) -> Result<()> {
        Ok(())
    }

    fn runner_fail(_: &Path, _: &Path) -> Result<()> {
        Err(anyhow!("boom"))
    }

    static MKDIR_FAIL_RUNNER_CALLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    fn runner_mark_called(_: &Path, _: &Path) -> Result<()> {
        MKDIR_FAIL_RUNNER_CALLED.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    #[test]
    fn execute_skips_when_output_exists() {
        let dir = test_dir("skip-existing");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");
        fs::write(&output, b"").expect("create output");

        let config = test_config(false, false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output,
            }],
            runner_fail,
        );

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Skipped));
        assert_eq!(report.workers, 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_converts_existing_output_when_overwrite_is_enabled() {
        let dir = test_dir("overwrite-existing");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");
        fs::write(&output, b"").expect("create output");

        let config = test_config(false, true);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output,
            }],
            runner_ok,
        );

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Converted));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_dry_run_marks_converted_without_calling_runner() {
        fn panic_runner(_: &Path, _: &Path) -> Result<()> {
            panic!("runner should not be called during dry-run");
        }

        let dir = test_dir("dry-run");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(true, false);
        let report =
            execute_with_runner(&config, vec![ConversionJob { input, output }], panic_runner);

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Converted));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_maps_runner_error_to_failed_result() {
        let dir = test_dir("runner-fail");
        let input = dir.join("song.flac");
        let output = dir.join("nested/song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false, false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output: output.clone(),
            }],
            runner_fail,
        );

        assert_eq!(report.results.len(), 1);
        match &report.results[0] {
            JobResult::Failed {
                input: failed_input,
                error,
            } => {
                assert_eq!(failed_input, &input);
                assert!(error.contains("boom"));
                assert!(error.contains(&input.display().to_string()));
                assert!(error.contains(&output.display().to_string()));
            }
            _ => panic!("expected failed result"),
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_returns_failed_when_output_parent_is_not_directory() {
        let dir = test_dir("mkdir-fail");
        let input = dir.join("song.flac");
        let blocked_parent = dir.join("blocked");
        let output = blocked_parent.join("song.aiff");
        fs::write(&input, b"").expect("create input");
        fs::write(&blocked_parent, b"").expect("create file that blocks directory creation");

        MKDIR_FAIL_RUNNER_CALLED.store(false, std::sync::atomic::Ordering::Relaxed);

        let config = test_config(false, false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output: output.clone(),
            }],
            runner_mark_called,
        );

        assert_eq!(report.results.len(), 1);
        match &report.results[0] {
            JobResult::Failed {
                input: failed_input,
                error,
            } => {
                assert_eq!(failed_input, &input);
                assert!(error.contains("failed to create output directory"));
                assert!(error.contains(&blocked_parent.display().to_string()));
            }
            _ => panic!("expected failed result"),
        }
        assert!(!MKDIR_FAIL_RUNNER_CALLED.load(Ordering::Relaxed));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_successfully_converts_when_runner_succeeds() {
        let dir = test_dir("runner-ok");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false, false);
        let report = execute_with_runner(&config, vec![ConversionJob { input, output }], runner_ok);

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Converted));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_reports_zero_workers_for_empty_job_list() {
        let config = test_config(true, false);
        let report = execute_with_runner(&config, Vec::new(), runner_ok);

        assert_eq!(report.results.len(), 0);
        assert_eq!(report.workers, 0);
    }

    #[test]
    fn execute_reports_actual_workers_used() {
        let mut config = test_config(true, false);
        config.jobs = 8;

        let report = execute_with_runner(
            &config,
            vec![
                ConversionJob {
                    input: PathBuf::from("first.flac"),
                    output: PathBuf::from("first.aiff"),
                },
                ConversionJob {
                    input: PathBuf::from("second.flac"),
                    output: PathBuf::from("second.aiff"),
                },
            ],
            runner_ok,
        );

        assert_eq!(report.results.len(), 2);
        assert_eq!(report.workers, 2);
    }
}
