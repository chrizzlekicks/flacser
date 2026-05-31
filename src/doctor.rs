use crate::{config, ffmpeg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
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
            .all(|check| check.status == CheckStatus::Ok)
    }

    pub fn print(&self) {
        println!("Doctor report:");

        for check in &self.checks {
            let marker = match check.status {
                CheckStatus::Ok => "ok",
                CheckStatus::Failed => "fail",
            };
            println!("[{marker}] {}: {}", check.name, check.detail);
        }

        println!("Read-only: no files were created, modified, or converted.");
        println!("Ready: {}", if self.is_ready() { "yes" } else { "no" });
    }
}

pub fn run() -> DoctorReport {
    diagnose(ffmpeg::probe_version, config::detected_cpu_cores)
}

fn diagnose(
    ffmpeg_probe: impl FnOnce() -> ffmpeg::VersionProbe,
    cpu_cores: impl FnOnce() -> usize,
) -> DoctorReport {
    let version_probe = ffmpeg_probe();
    let mut checks = Vec::new();

    if version_probe.executable_found {
        checks.push(DoctorCheck {
            name: "ffmpeg",
            status: CheckStatus::Ok,
            detail: "found".to_string(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "ffmpeg",
            status: CheckStatus::Failed,
            detail: "not found".to_string(),
        });
    }

    match version_probe.version {
        Ok(version) => checks.push(DoctorCheck {
            name: "ffmpeg version",
            status: CheckStatus::Ok,
            detail: version,
        }),
        Err(error) => checks.push(DoctorCheck {
            name: "ffmpeg version",
            status: CheckStatus::Failed,
            detail: format!("unreadable ({error})"),
        }),
    }

    let cores = cpu_cores();
    checks.push(DoctorCheck {
        name: "cpu cores",
        status: CheckStatus::Ok,
        detail: cores.to_string(),
    });

    checks.push(DoctorCheck {
        name: "default workers",
        status: CheckStatus::Ok,
        detail: config::default_jobs_for_cpu_count(cores).to_string(),
    });

    let config_sane = cores >= 1 && config::default_jobs_for_cpu_count(cores) >= 1;
    checks.push(DoctorCheck {
        name: "config sanity",
        status: if config_sane {
            CheckStatus::Ok
        } else {
            CheckStatus::Failed
        },
        detail: if config_sane {
            "global defaults are sane".to_string()
        } else {
            "global defaults are not sane".to_string()
        },
    });

    DoctorReport { checks }
}

#[cfg(test)]
mod tests {
    use crate::ffmpeg::VersionProbe;

    use super::{CheckStatus, diagnose};

    #[test]
    fn reports_ready_when_required_global_checks_pass() {
        let report = diagnose(
            || VersionProbe {
                executable_found: true,
                version: Ok("ffmpeg version 7.1".to_string()),
            },
            || 8,
        );

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
}
