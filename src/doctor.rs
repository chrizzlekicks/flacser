use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

use crate::{
    audio_format::AudioFormat,
    cli::DoctorArgs,
    config::{self, Config},
    discover, ffmpeg, plan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn is_ready(&self) -> bool {
        self.checks
            .iter()
            .all(|check| check.status != CheckStatus::Failed)
    }

    pub fn has_warnings(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == CheckStatus::Warning)
    }

    fn push(&mut self, check: DoctorCheck) {
        self.checks.push(check);
    }

    pub fn print(&self) {
        println!("Doctor report:");

        for check in &self.checks {
            let marker = match check.status {
                CheckStatus::Ok => "ok",
                CheckStatus::Warning => "warn",
                CheckStatus::Failed => "fail",
            };
            println!("[{marker}] {}: {}", check.name, check.detail);
        }

        println!("Read-only: no files were created, modified, or converted.");
        println!(
            "Warnings: {}",
            if self.has_warnings() { "yes" } else { "no" }
        );
        println!("Ready: {}", if self.is_ready() { "yes" } else { "no" });
    }
}

impl DoctorCheck {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Ok,
            detail: detail.into(),
        }
    }

    fn warning(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warning,
            detail: detail.into(),
        }
    }

    fn failed(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Failed,
            detail: detail.into(),
        }
    }
}

pub fn run(args: DoctorArgs) -> DoctorReport {
    diagnose(
        DoctorInput::from_args(args),
        ffmpeg::probe_version,
        ffmpeg::probe_ffprobe_version,
        config::detected_cpu_cores,
    )
}

fn diagnose(
    input: DoctorInput,
    ffmpeg_probe: impl FnOnce() -> ffmpeg::VersionProbe,
    ffprobe_probe: impl FnOnce() -> ffmpeg::VersionProbe,
    cpu_cores: impl FnOnce() -> usize,
) -> DoctorReport {
    let version_probe = ffmpeg_probe();
    let ffprobe_version_probe = ffprobe_probe();
    let mut report = DoctorReport { checks: Vec::new() };

    if version_probe.executable_found {
        report.push(DoctorCheck::ok("ffmpeg", "found"));
    } else {
        report.push(DoctorCheck::failed("ffmpeg", "not found"));
    }

    match version_probe.version {
        Ok(version) => report.push(DoctorCheck::ok("ffmpeg version", version)),
        Err(error) => report.push(DoctorCheck::failed(
            "ffmpeg version",
            format!("unreadable ({error})"),
        )),
    }

    if ffprobe_version_probe.executable_found {
        report.push(DoctorCheck::ok("ffprobe", "found"));
    } else {
        report.push(DoctorCheck::failed("ffprobe", "not found"));
    }

    match ffprobe_version_probe.version {
        Ok(version) => report.push(DoctorCheck::ok("ffprobe version", version)),
        Err(error) => report.push(DoctorCheck::failed(
            "ffprobe version",
            format!("unreadable ({error})"),
        )),
    }

    let cores = cpu_cores();
    report.push(DoctorCheck::ok("cpu cores", cores.to_string()));

    report.push(DoctorCheck::ok(
        "default workers",
        config::default_jobs_for_cpu_count(cores).to_string(),
    ));

    let config_sane = cores >= 1 && config::default_jobs_for_cpu_count(cores) >= 1;
    report.push(if config_sane {
        DoctorCheck::ok("config sanity", "global defaults are sane")
    } else {
        DoctorCheck::failed("config sanity", "global defaults are not sane")
    });

    if let Some(jobs) = input.jobs {
        report.push(DoctorCheck::ok("configured workers", jobs.to_string()));
        if jobs > cores {
            report.push(DoctorCheck::warning(
                "worker oversubscription",
                format!(
                    "{jobs} configured workers exceeds {cores} detected CPU cores; each job runs its own ffmpeg process"
                ),
            ));
        }
    }

    if let Some(output_dir) = input.output_dir.as_deref() {
        report.push(check_output_dir(output_dir));
    }

    if let Some(input_path) = input.input_path.as_ref() {
        add_input_checks(&mut report, &input, input_path);
    }

    report
}

#[derive(Debug, Clone)]
struct DoctorInput {
    input_path: Option<PathBuf>,
    target_format: Option<AudioFormat>,
    output_dir: Option<PathBuf>,
    jobs: Option<usize>,
}

impl DoctorInput {
    fn from_args(args: DoctorArgs) -> Self {
        Self {
            input_path: args.input_path,
            target_format: args.to,
            output_dir: args.output_dir,
            jobs: args.jobs.map(NonZeroUsize::get),
        }
    }

    fn discovery_config(&self, input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: self.output_dir.clone(),
            overwrite: false,
            dry_run: true,
            recursive: false,
            jobs: self.jobs.unwrap_or_else(config::default_jobs),
            target_format: self.target_format.unwrap_or(AudioFormat::Flac),
        }
    }
}

fn add_input_checks(report: &mut DoctorReport, input: &DoctorInput, input_path: &Path) {
    if input_path.is_file() {
        report.push(DoctorCheck::ok(
            "input exists",
            input_path.display().to_string(),
        ));
        report.push(DoctorCheck::ok("input type", "file"));
    } else if input_path.is_dir() {
        report.push(DoctorCheck::ok(
            "input exists",
            input_path.display().to_string(),
        ));
        report.push(DoctorCheck::ok("input type", "directory"));
    } else {
        report.push(DoctorCheck::failed(
            "input exists",
            format!("not found or not accessible: {}", input_path.display()),
        ));
        return;
    }

    let config = input.discovery_config(input_path.to_path_buf());
    let inputs = match discover_inputs_for_doctor(input, &config) {
        Ok(inputs) => inputs,
        Err(error) => {
            report.push(DoctorCheck::failed("discoverable files", error.to_string()));
            return;
        }
    };

    if inputs.is_empty() {
        report.push(DoctorCheck::failed(
            "discoverable files",
            "0 supported audio files found with non-recursive discovery",
        ));
        report.push(DoctorCheck::ok("effective workers", "0"));
        return;
    }

    report.push(DoctorCheck::ok(
        "discoverable files",
        format!(
            "{} supported audio file(s) found with non-recursive discovery",
            inputs.len()
        ),
    ));

    let job_count = inputs.len();
    match validate_output_planning(input, &config, inputs) {
        Ok(planned_outputs) => {
            report.push(DoctorCheck::ok(
                "output planning",
                format!("{planned_outputs} output path(s) validated"),
            ));
            report.push(DoctorCheck::ok(
                "effective workers",
                std::cmp::min(job_count, config.jobs).to_string(),
            ));
        }
        Err(error) => report.push(DoctorCheck::failed("output planning", error.to_string())),
    }
}

fn discover_inputs_for_doctor(input: &DoctorInput, config: &Config) -> Result<Vec<PathBuf>> {
    match input.target_format {
        Some(_) => discover::discover(config),
        None => discover::discover_for_doctor(config.input_path.as_path(), false),
    }
}

fn validate_output_planning(
    input: &DoctorInput,
    config: &Config,
    inputs: Vec<PathBuf>,
) -> Result<usize> {
    if input.target_format.is_none() {
        bail!("target format is required; pass --to <format> or set FLACSER_CONVERT_TO");
    }

    let jobs = plan::plan(config, inputs)?;
    Ok(jobs.len())
}

fn check_output_dir(output_dir: &Path) -> DoctorCheck {
    match fs::metadata(output_dir) {
        Ok(metadata) if metadata.is_dir() => {
            if is_writable_metadata(&metadata) {
                DoctorCheck::ok(
                    "output directory",
                    format!("writable: {}", output_dir.display()),
                )
            } else {
                DoctorCheck::failed(
                    "output directory",
                    format!("not writable: {}", output_dir.display()),
                )
            }
        }
        Ok(_) => DoctorCheck::failed(
            "output directory",
            format!("exists but is not a directory: {}", output_dir.display()),
        ),
        Err(_) => match nearest_existing_parent(output_dir) {
            Some(parent) if is_writable_path(&parent) => DoctorCheck::ok(
                "output directory",
                format!("createable under existing parent: {}", parent.display()),
            ),
            Some(parent) => DoctorCheck::failed(
                "output directory",
                format!("parent is not writable: {}", parent.display()),
            ),
            None => DoctorCheck::failed(
                "output directory",
                format!("no existing parent found for {}", output_dir.display()),
            ),
        },
    }
}

fn nearest_existing_parent(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|ancestor| ancestor.exists())
        .map(Path::to_path_buf)
}

fn is_writable_path(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| is_writable_metadata(&metadata))
}

#[cfg(unix)]
fn is_writable_metadata(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o222 != 0
}

#[cfg(not(unix))]
fn is_writable_metadata(metadata: &fs::Metadata) -> bool {
    !metadata.permissions().readonly()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        num::NonZeroUsize,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::ffmpeg::VersionProbe;
    use crate::{audio_format::AudioFormat, cli::DoctorArgs};

    use super::{CheckStatus, DoctorCheck, DoctorInput, DoctorReport, diagnose};

    fn test_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "flacser-doctor-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn args(
        input_path: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> DoctorInput {
        args_to(input_path, Some(AudioFormat::Aiff), output_dir, jobs)
    }

    fn args_to(
        input_path: Option<PathBuf>,
        target_format: Option<AudioFormat>,
        output_dir: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> DoctorInput {
        DoctorInput::from_args(DoctorArgs {
            input_path,
            to: target_format,
            output_dir,
            jobs: jobs.and_then(NonZeroUsize::new),
        })
    }

    fn args_without_target(
        input_path: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        jobs: Option<usize>,
    ) -> DoctorInput {
        DoctorInput::from_args(DoctorArgs {
            input_path,
            to: None,
            output_dir,
            jobs: jobs.and_then(NonZeroUsize::new),
        })
    }

    fn passing_probe() -> VersionProbe {
        VersionProbe {
            executable_found: true,
            version: Ok("ffmpeg version 7.1".to_string()),
        }
    }

    fn passing_ffprobe_probe() -> VersionProbe {
        VersionProbe {
            executable_found: true,
            version: Ok("ffprobe version 7.1".to_string()),
        }
    }

    fn check<'a>(report: &'a DoctorReport, name: &str) -> &'a DoctorCheck {
        report
            .checks
            .iter()
            .find(|check| check.name == name)
            .expect("check should exist")
    }

    #[test]
    fn reports_ready_when_required_global_checks_pass() {
        let report = diagnose(
            args(None, None, None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert!(report.is_ready());
        assert_eq!(check(&report, "ffmpeg").status, CheckStatus::Ok);
        assert_eq!(check(&report, "ffmpeg").detail, "found");
        assert_eq!(
            check(&report, "ffmpeg version").detail,
            "ffmpeg version 7.1"
        );
        assert_eq!(check(&report, "ffprobe").status, CheckStatus::Ok);
        assert_eq!(check(&report, "ffprobe").detail, "found");
        assert_eq!(
            check(&report, "ffprobe version").detail,
            "ffprobe version 7.1"
        );
        assert_eq!(check(&report, "cpu cores").detail, "8");
        assert_eq!(check(&report, "default workers").detail, "7");
        assert_eq!(
            check(&report, "config sanity").detail,
            "global defaults are sane"
        );
    }

    #[test]
    fn reports_not_ready_when_ffmpeg_is_missing() {
        let report = diagnose(
            args(None, None, None),
            || VersionProbe {
                executable_found: false,
                version: Err("failed to run ffmpeg -version".to_string()),
            },
            passing_ffprobe_probe,
            || 4,
        );

        assert!(!report.is_ready());
        assert_eq!(report.checks[0].status, CheckStatus::Failed);
        assert_eq!(report.checks[0].detail, "not found");
        assert_eq!(report.checks[1].status, CheckStatus::Failed);
        assert!(
            report.checks[1]
                .detail
                .contains("failed to run ffmpeg -version")
        );
        assert_eq!(check(&report, "default workers").detail, "3");
    }

    #[test]
    fn reports_found_ffmpeg_when_only_version_check_fails() {
        let report = diagnose(
            args(None, None, None),
            || VersionProbe {
                executable_found: true,
                version: Err("ffmpeg -version exited with status 42".to_string()),
            },
            passing_ffprobe_probe,
            || 4,
        );

        assert!(!report.is_ready());
        assert_eq!(report.checks[0].status, CheckStatus::Ok);
        assert_eq!(report.checks[0].detail, "found");
        assert_eq!(report.checks[1].status, CheckStatus::Failed);
        assert!(
            report.checks[1]
                .detail
                .contains("ffmpeg -version exited with status 42")
        );
    }

    #[test]
    fn reports_not_ready_when_ffprobe_is_missing() {
        let report = diagnose(
            args_without_target(None, None, None),
            passing_probe,
            || VersionProbe {
                executable_found: false,
                version: Err("failed to run ffprobe -version".to_string()),
            },
            || 4,
        );

        assert!(!report.is_ready());
        assert_eq!(check(&report, "ffprobe").status, CheckStatus::Failed);
        assert_eq!(check(&report, "ffprobe").detail, "not found");
        assert_eq!(
            check(&report, "ffprobe version").status,
            CheckStatus::Failed
        );
        assert!(
            check(&report, "ffprobe version")
                .detail
                .contains("failed to run ffprobe -version")
        );
    }

    #[test]
    fn warnings_do_not_make_report_not_ready() {
        let report = DoctorReport {
            checks: vec![DoctorCheck::warning("configured workers", "high")],
        };

        assert!(report.has_warnings());
        assert!(report.is_ready());
    }

    #[test]
    fn reports_configured_workers_when_jobs_is_provided() {
        let report = diagnose(
            args(None, None, Some(3)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "configured workers").detail, "3");
        assert!(report.is_ready());
    }

    #[test]
    fn warns_when_configured_workers_exceed_detected_cpu_cores() {
        let report = diagnose(
            args(None, None, Some(20)),
            passing_probe,
            passing_ffprobe_probe,
            || 4,
        );

        assert_eq!(
            check(&report, "worker oversubscription").status,
            CheckStatus::Warning
        );
        assert!(report.has_warnings());
        assert!(report.is_ready());
    }

    #[test]
    fn valid_file_input_reports_discovery_plan_and_workers() {
        let dir = test_dir("file-input");
        let input = dir.join("song.flac");
        fs::write(&input, b"").expect("create input");

        let report = diagnose(
            args(Some(input.clone()), None, Some(2)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "input type").detail, "file");
        assert_eq!(check(&report, "discoverable files").status, CheckStatus::Ok);
        assert_eq!(
            check(&report, "output planning").detail,
            "1 output path(s) validated"
        );
        assert_eq!(check(&report, "effective workers").detail, "1");
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn valid_aiff_file_input_does_not_fail_same_format_planning() {
        let dir = test_dir("aiff-file-input");
        let input = dir.join("song.aiff");
        fs::write(&input, b"").expect("create input");

        let report = diagnose(
            args_to(Some(input), Some(AudioFormat::Flac), None, Some(2)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "discoverable files").status, CheckStatus::Ok);
        assert_eq!(check(&report, "output planning").status, CheckStatus::Ok);
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn input_without_target_fails_output_planning() {
        let dir = test_dir("targetless-file-input");
        let input = dir.join("song.flac");
        fs::write(&input, b"").expect("create input");

        let report = diagnose(
            args_without_target(Some(input), None, Some(2)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "discoverable files").status, CheckStatus::Ok);
        assert_eq!(
            check(&report, "output planning").status,
            CheckStatus::Failed
        );
        assert!(
            check(&report, "output planning")
                .detail
                .contains("target format is required")
        );
        assert!(!report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn valid_directory_input_uses_non_recursive_discovery() {
        let dir = test_dir("dir-input");
        fs::write(dir.join("top.flac"), b"").expect("create top-level input");
        fs::create_dir_all(dir.join("nested")).expect("create nested");
        fs::write(dir.join("nested/inner.flac"), b"").expect("create nested input");

        let report = diagnose(
            args(Some(dir.clone()), None, Some(4)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "input type").detail, "directory");
        assert!(
            check(&report, "discoverable files")
                .detail
                .starts_with("1 supported audio")
        );
        assert_eq!(check(&report, "effective workers").detail, "1");
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn valid_directory_input_keeps_target_format_files_discoverable_for_doctor() {
        let dir = test_dir("dir-input-mixed");
        fs::write(dir.join("top.flac"), b"").expect("create top-level flac input");
        fs::write(dir.join("top.aiff"), b"").expect("create top-level aiff input");

        let report = diagnose(
            args(Some(dir.clone()), None, Some(4)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "discoverable files").status, CheckStatus::Ok);
        assert!(
            check(&report, "discoverable files")
                .detail
                .starts_with("1 supported audio")
        );
        assert_eq!(check(&report, "output planning").status, CheckStatus::Ok);
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn mixed_directory_detects_convert_output_collision_for_target() {
        let dir = test_dir("dir-output-collision");
        fs::write(dir.join("song.flac"), b"").expect("create flac input");
        fs::write(dir.join("song.wav"), b"").expect("create wav input");

        let report = diagnose(
            args(Some(dir.clone()), None, Some(4)),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(
            check(&report, "output planning").status,
            CheckStatus::Failed
        );
        assert!(
            check(&report, "output planning")
                .detail
                .contains("output collision detected")
        );
        assert!(!report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_directory_fails_discoverable_files() {
        let dir = test_dir("empty-dir");

        let report = diagnose(
            args(Some(dir.clone()), None, None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(
            check(&report, "discoverable files").status,
            CheckStatus::Failed
        );
        assert_eq!(check(&report, "effective workers").detail, "0");
        assert!(!report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_input_fails_input_diagnostics() {
        let missing = test_dir("missing-input").join("missing");

        let report = diagnose(
            args(Some(missing), None, None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "input exists").status, CheckStatus::Failed);
        assert!(!report.is_ready());
    }

    #[test]
    fn unsupported_file_fails_discovery() {
        let dir = test_dir("unsupported");
        let input = dir.join("song.mp3");
        fs::write(&input, b"").expect("create input");

        let report = diagnose(
            args(Some(input), None, None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(
            check(&report, "discoverable files").status,
            CheckStatus::Failed
        );
        assert!(!report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn output_dir_existing_directory_passes() {
        let dir = test_dir("output-dir");

        let report = diagnose(
            args(None, Some(dir.clone()), None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "output directory").status, CheckStatus::Ok);
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn output_dir_existing_file_fails() {
        let dir = test_dir("output-file");
        let output = dir.join("out");
        fs::write(&output, b"").expect("create output file");

        let report = diagnose(
            args(None, Some(output), None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(
            check(&report, "output directory").status,
            CheckStatus::Failed
        );
        assert!(!report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_output_dir_reports_createable_when_parent_is_writable() {
        let dir = test_dir("missing-output-dir");
        let output = dir.join("out").join("nested");

        let report = diagnose(
            args(None, Some(output), None),
            passing_probe,
            passing_ffprobe_probe,
            || 8,
        );

        assert_eq!(check(&report, "output directory").status, CheckStatus::Ok);
        assert!(
            check(&report, "output directory")
                .detail
                .contains(&dir.display().to_string())
        );
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }
}
