use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use crate::{
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
        config::detected_cpu_cores,
    )
}

fn diagnose(
    input: DoctorInput,
    ffmpeg_probe: impl FnOnce() -> ffmpeg::VersionProbe,
    cpu_cores: impl FnOnce() -> usize,
) -> DoctorReport {
    let version_probe = ffmpeg_probe();
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
    output_dir: Option<PathBuf>,
    jobs: Option<usize>,
}

impl DoctorInput {
    fn from_args(args: DoctorArgs) -> Self {
        Self {
            input_path: args.input_path,
            output_dir: args.output_dir,
            jobs: args.jobs.map(NonZeroUsize::get),
        }
    }

    fn config(&self, input_path: PathBuf) -> Config {
        Config {
            input_path,
            output_dir: self.output_dir.clone(),
            overwrite: false,
            dry_run: true,
            recursive: false,
            flatten: false,
            jobs: self.jobs.unwrap_or_else(config::default_jobs),
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

    let config = input.config(input_path.to_path_buf());
    let inputs = match discover::discover(&config) {
        Ok(inputs) => inputs,
        Err(error) => {
            report.push(DoctorCheck::failed("discoverable files", error.to_string()));
            return;
        }
    };

    if inputs.is_empty() {
        report.push(DoctorCheck::failed(
            "discoverable files",
            "0 .flac files found with non-recursive discovery",
        ));
        report.push(DoctorCheck::ok("effective workers", "0"));
        return;
    }

    report.push(DoctorCheck::ok(
        "discoverable files",
        format!(
            "{} .flac file(s) found with non-recursive discovery",
            inputs.len()
        ),
    ));

    let job_count = inputs.len();
    match plan::plan(&config, inputs) {
        Ok(jobs) => {
            report.push(DoctorCheck::ok(
                "output planning",
                format!("{} output path(s) validated", jobs.len()),
            ));
            report.push(DoctorCheck::ok(
                "effective workers",
                std::cmp::min(job_count, config.jobs).to_string(),
            ));
        }
        Err(error) => report.push(DoctorCheck::failed("output planning", error.to_string())),
    }
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

    use crate::cli::DoctorArgs;
    use crate::ffmpeg::VersionProbe;

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
        DoctorInput::from_args(DoctorArgs {
            input_path,
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

    fn check<'a>(report: &'a DoctorReport, name: &str) -> &'a DoctorCheck {
        report
            .checks
            .iter()
            .find(|check| check.name == name)
            .expect("check should exist")
    }

    #[test]
    fn reports_ready_when_required_global_checks_pass() {
        let report = diagnose(args(None, None, None), passing_probe, || 8);

        assert!(report.is_ready());
        assert_eq!(report.checks[0].status, CheckStatus::Ok);
        assert_eq!(report.checks[0].detail, "found");
        assert_eq!(report.checks[1].detail, "ffmpeg version 7.1");
        assert_eq!(report.checks[2].detail, "8");
        assert_eq!(report.checks[3].detail, "7");
        assert_eq!(report.checks[4].detail, "global defaults are sane");
    }

    #[test]
    fn reports_not_ready_when_ffmpeg_is_missing() {
        let report = diagnose(
            args(None, None, None),
            || VersionProbe {
                executable_found: false,
                version: Err("failed to run ffmpeg -version".to_string()),
            },
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
        assert_eq!(report.checks[3].detail, "3");
    }

    #[test]
    fn reports_found_ffmpeg_when_only_version_check_fails() {
        let report = diagnose(
            args(None, None, None),
            || VersionProbe {
                executable_found: true,
                version: Err("ffmpeg -version exited with status 42".to_string()),
            },
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
    fn warnings_do_not_make_report_not_ready() {
        let report = DoctorReport {
            checks: vec![DoctorCheck::warning("configured workers", "high")],
        };

        assert!(report.has_warnings());
        assert!(report.is_ready());
    }

    #[test]
    fn reports_configured_workers_when_jobs_is_provided() {
        let report = diagnose(args(None, None, Some(3)), passing_probe, || 8);

        assert_eq!(check(&report, "configured workers").detail, "3");
        assert!(report.is_ready());
    }

    #[test]
    fn warns_when_configured_workers_exceed_detected_cpu_cores() {
        let report = diagnose(args(None, None, Some(20)), passing_probe, || 4);

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
    fn valid_directory_input_uses_non_recursive_discovery() {
        let dir = test_dir("dir-input");
        fs::write(dir.join("top.flac"), b"").expect("create top-level input");
        fs::create_dir_all(dir.join("nested")).expect("create nested");
        fs::write(dir.join("nested/inner.flac"), b"").expect("create nested input");

        let report = diagnose(args(Some(dir.clone()), None, Some(4)), passing_probe, || 8);

        assert_eq!(check(&report, "input type").detail, "directory");
        assert!(
            check(&report, "discoverable files")
                .detail
                .starts_with("1 .flac")
        );
        assert_eq!(check(&report, "effective workers").detail, "1");
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_directory_fails_discoverable_files() {
        let dir = test_dir("empty-dir");

        let report = diagnose(args(Some(dir.clone()), None, None), passing_probe, || 8);

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

        let report = diagnose(args(Some(missing), None, None), passing_probe, || 8);

        assert_eq!(check(&report, "input exists").status, CheckStatus::Failed);
        assert!(!report.is_ready());
    }

    #[test]
    fn non_flac_file_fails_discovery() {
        let dir = test_dir("non-flac");
        let input = dir.join("song.wav");
        fs::write(&input, b"").expect("create input");

        let report = diagnose(args(Some(input), None, None), passing_probe, || 8);

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

        let report = diagnose(args(None, Some(dir.clone()), None), passing_probe, || 8);

        assert_eq!(check(&report, "output directory").status, CheckStatus::Ok);
        assert!(report.is_ready());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn output_dir_existing_file_fails() {
        let dir = test_dir("output-file");
        let output = dir.join("out");
        fs::write(&output, b"").expect("create output file");

        let report = diagnose(args(None, Some(output), None), passing_probe, || 8);

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

        let report = diagnose(args(None, Some(output), None), passing_probe, || 8);

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
