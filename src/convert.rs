use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;

use rayon::prelude::*;

use crate::audio_format::AudioFormat;
use crate::config::Config;
use crate::ffmpeg::run_ffmpeg;
use crate::interrupt::InterruptFlag;
use crate::plan::ConversionJob;
use crate::progress::ProgressReporter;

type FfmpegRunner = fn(&ConversionJob, &Path) -> Result<()>;

#[derive(Debug, Clone)]
pub enum JobResult {
    Converted,
    Skipped,
    Interrupted { input: PathBuf },
    Failed { input: PathBuf, error: String },
}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub results: Vec<JobResult>,
    pub workers: usize,
    pub interrupted: bool,
}

pub fn execute(
    config: &Config,
    jobs: Vec<ConversionJob>,
    interrupt: &InterruptFlag,
) -> ExecutionReport {
    execute_with_runner(config, jobs, run_ffmpeg, interrupt)
}

fn execute_with_runner(
    config: &Config,
    jobs: Vec<ConversionJob>,
    runner: FfmpegRunner,
    interrupt: &InterruptFlag,
) -> ExecutionReport {
    let configured_jobs = config.jobs;
    let dry_run = config.dry_run;

    if jobs.is_empty() {
        return ExecutionReport {
            results: Vec::new(),
            workers: 0,
            interrupted: interrupt.is_interrupted(),
        };
    }

    let reporter = ProgressReporter::new(jobs.len());

    let workers = configured_jobs.min(jobs.len()).max(1);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .expect("failed to build rayon threadpool");

    if jobs.iter().any(|j| j.target_format == AudioFormat::Wav) {
        eprintln!("{}", crate::ffmpeg::wav_metadata_note().unwrap());
    }

    let results: Vec<JobResult> = pool.install(|| {
        jobs.into_par_iter()
            .map(|job| {
                let result = if interrupt.is_interrupted() {
                    JobResult::Interrupted { input: job.input }
                } else {
                    execute_job(job, dry_run, runner)
                };
                reporter.finish_job();
                result
            })
            .collect()
    });

    let interrupted = interrupt.is_interrupted()
        || results
            .iter()
            .any(|result| matches!(result, JobResult::Interrupted { .. }));

    ExecutionReport {
        results,
        workers,
        interrupted,
    }
}

fn temp_output_path(output: &Path) -> PathBuf {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = output
        .file_name()
        .expect("planned output path should have a file name");

    let mut temp_file_name = format!(".{}.flacser-{pid}-{id}", file_name.to_string_lossy());

    #[expect(
        clippy::single_match,
        reason = "match keeps both Option cases explicit in this path-building helper"
    )]
    match output.extension() {
        Some(extension) => {
            temp_file_name.push('.');
            temp_file_name.push_str(&extension.to_string_lossy());
        }
        None => {}
    }

    output.with_file_name(temp_file_name)
}

fn execute_job(job: ConversionJob, dry_run: bool, runner: FfmpegRunner) -> JobResult {
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

    let temp_output = temp_output_path(&job.output);

    match runner(&job, &temp_output) {
        Ok(()) => match fs::rename(&temp_output, &job.output) {
            Ok(()) => JobResult::Converted,
            Err(error) => {
                let _ = fs::remove_file(&temp_output);
                JobResult::Failed {
                    input: job.input.clone(),
                    error: format!(
                        "failed to move temporary output {} to {}: {error}",
                        temp_output.display(),
                        job.output.display()
                    ),
                }
            }
        },
        Err(error) => {
            let _ = fs::remove_file(&temp_output);
            JobResult::Failed {
                input: job.input.clone(),
                error: format!(
                    "{} -> {}: {error:#}",
                    job.input.display(),
                    job.output.display()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, AtomicUsize, Ordering},
        thread,
        time::Duration,
    };

    use anyhow::{Result, anyhow};

    use crate::{
        audio_format::AudioFormat, config::Config, interrupt::InterruptFlag, plan::ConversionJob,
    };

    use super::{JobResult, execute_with_runner, temp_output_path};

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

    fn test_config(dry_run: bool) -> Config {
        Config {
            input_path: PathBuf::from("ignored"),
            output_dir: None,
            dry_run,
            recursive: false,
            flatten: false,
            jobs: 1,
            target_format: AudioFormat::Aiff,
        }
    }

    fn interrupt_flag() -> InterruptFlag {
        InterruptFlag::new()
    }

    fn runner_ok(_: &ConversionJob, output: &Path) -> Result<()> {
        fs::write(output, b"converted")?;
        Ok(())
    }

    fn runner_fail(_: &ConversionJob, _: &Path) -> Result<()> {
        Err(anyhow!("boom"))
    }

    fn runner_write_partial_then_fail(_: &ConversionJob, output: &Path) -> Result<()> {
        fs::write(output, b"partial")?;
        Err(anyhow!("boom"))
    }

    fn temp_files_in(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("read entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(".flacser-"))
            })
            .collect()
    }

    static MKDIR_FAIL_RUNNER_CALLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    static INTERRUPT_MID_BATCH_RUNNER_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn runner_mark_called(_: &ConversionJob, _: &Path) -> Result<()> {
        MKDIR_FAIL_RUNNER_CALLED.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn runner_slow_ok(_: &ConversionJob, output: &Path) -> Result<()> {
        INTERRUPT_MID_BATCH_RUNNER_CALLS.fetch_add(1, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(100));
        fs::write(output, b"converted")?;
        Ok(())
    }

    #[test]
    fn execute_skips_when_output_exists() {
        let dir = test_dir("skip-existing");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");
        fs::write(&output, b"").expect("create output");

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output,
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_fail,
            &interrupt_flag(),
        );

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Skipped));
        assert_eq!(report.workers, 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_dry_run_marks_converted_without_calling_runner() {
        fn panic_runner(_: &ConversionJob, _: &Path) -> Result<()> {
            panic!("runner should not be called during dry-run");
        }

        let dir = test_dir("dry-run");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(true);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input,
                output,
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            panic_runner,
            &interrupt_flag(),
        );

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

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output: output.clone(),
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_fail,
            &interrupt_flag(),
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

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output: output.clone(),
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_mark_called,
            &interrupt_flag(),
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
    fn temp_output_path_uses_output_file_name_in_same_directory() {
        let output = PathBuf::from("album/song.aiff");

        let temp = temp_output_path(&output);

        assert_eq!(temp.parent(), output.parent());
        let name = temp
            .file_name()
            .and_then(|name| name.to_str())
            .expect("temp file name should be utf-8 in test");
        assert!(name.starts_with(".song.aiff.flacser-"));
        assert!(name.ends_with(".aiff"));
    }

    #[test]
    fn successful_conversion_renames_temp_output_to_final_output() {
        let dir = test_dir("temp-rename-success");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input,
                output: output.clone(),
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_ok,
            &interrupt_flag(),
        );

        assert!(matches!(report.results[0], JobResult::Converted));
        assert_eq!(fs::read(&output).expect("read final output"), b"converted");
        assert!(temp_files_in(&dir).is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_conversion_removes_partial_temp_output_without_final_output() {
        let dir = test_dir("temp-cleanup-fail");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input,
                output: output.clone(),
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_write_partial_then_fail,
            &interrupt_flag(),
        );

        assert!(matches!(report.results[0], JobResult::Failed { .. }));
        assert!(!output.exists());
        assert!(temp_files_in(&dir).is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn interrupted_execution_does_not_call_runner() {
        fn panic_runner(_: &ConversionJob, _: &Path) -> Result<()> {
            panic!("runner should not be called after interrupt");
        }

        let dir = test_dir("interrupted-before-job");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");
        let interrupt = interrupt_flag();
        interrupt.interrupt();

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input: input.clone(),
                output,
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            panic_runner,
            &interrupt,
        );

        assert!(report.interrupted);
        assert!(
            matches!(&report.results[0], JobResult::Interrupted { input: interrupted_input } if interrupted_input == &input)
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn interrupt_during_batch_leaves_queued_jobs_unstarted() {
        let dir = test_dir("interrupted-mid-batch");
        let interrupt = interrupt_flag();
        let interrupter_interrupt = interrupt.shared();
        INTERRUPT_MID_BATCH_RUNNER_CALLS.store(0, Ordering::SeqCst);

        let jobs: Vec<_> = (0..4)
            .map(|id| {
                let input = dir.join(format!("song-{id}.flac"));
                fs::write(&input, b"").expect("create input");
                ConversionJob {
                    input,
                    output: dir.join(format!("song-{id}.aiff")),
                    source_format: AudioFormat::Flac,
                    target_format: AudioFormat::Aiff,
                }
            })
            .collect();

        let interrupter = thread::spawn(move || {
            while INTERRUPT_MID_BATCH_RUNNER_CALLS.load(Ordering::SeqCst) == 0 {
                thread::sleep(Duration::from_millis(5));
            }
            interrupter_interrupt.interrupt();
        });

        let mut config = test_config(false);
        config.jobs = 1;
        let report = execute_with_runner(&config, jobs, runner_slow_ok, &interrupt);

        interrupter.join().expect("interrupt helper should finish");
        assert!(report.interrupted);
        assert_eq!(INTERRUPT_MID_BATCH_RUNNER_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(
            report
                .results
                .iter()
                .filter(|result| matches!(result, JobResult::Converted))
                .count(),
            1
        );
        assert_eq!(
            report
                .results
                .iter()
                .filter(|result| matches!(result, JobResult::Interrupted { .. }))
                .count(),
            3
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_successfully_converts_when_runner_succeeds() {
        let dir = test_dir("runner-ok");
        let input = dir.join("song.flac");
        let output = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input,
                output,
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Aiff,
            }],
            runner_ok,
            &interrupt_flag(),
        );

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Converted));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn execute_reports_zero_workers_for_empty_job_list() {
        let config = test_config(true);
        let report = execute_with_runner(&config, Vec::new(), runner_ok, &interrupt_flag());

        assert_eq!(report.results.len(), 0);
        assert_eq!(report.workers, 0);
    }

    #[test]
    fn execute_reports_actual_workers_used() {
        let mut config = test_config(true);
        config.jobs = 8;

        let report = execute_with_runner(
            &config,
            vec![
                ConversionJob {
                    input: PathBuf::from("first.flac"),
                    output: PathBuf::from("first.aiff"),
                    source_format: AudioFormat::Flac,
                    target_format: AudioFormat::Aiff,
                },
                ConversionJob {
                    input: PathBuf::from("second.flac"),
                    output: PathBuf::from("second.aiff"),
                    source_format: AudioFormat::Flac,
                    target_format: AudioFormat::Aiff,
                },
            ],
            runner_ok,
            &interrupt_flag(),
        );

        assert_eq!(report.results.len(), 2);
        assert_eq!(report.workers, 2);
    }

    #[test]
    fn execute_successfully_converts_wav_target() {
        let dir = test_dir("wav-target");
        let input = dir.join("song.flac");
        let output = dir.join("song.wav");
        fs::write(&input, b"").expect("create input");

        let config = test_config(false);
        let report = execute_with_runner(
            &config,
            vec![ConversionJob {
                input,
                output,
                source_format: AudioFormat::Flac,
                target_format: AudioFormat::Wav,
            }],
            runner_ok,
            &interrupt_flag(),
        );

        assert_eq!(report.results.len(), 1);
        assert!(matches!(report.results[0], JobResult::Converted));

        let _ = fs::remove_dir_all(dir);
    }
}
