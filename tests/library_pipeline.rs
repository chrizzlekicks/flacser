use flacser::{
    audio_format::AudioFormat, config::Config, interrupt::InterruptFlag, summary::Summary,
};

#[test]
fn external_consumer_can_run_dry_run_pipeline() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("song.flac");
    std::fs::write(&input, b"audio").unwrap();
    let config = Config {
        input_path: input,
        output_dir: None,
        dry_run: true,
        recursive: false,
        flatten: false,
        jobs: 1,
        target_format: AudioFormat::Wav,
    };
    let report = flacser::run_conversion(&config, &InterruptFlag::new()).unwrap();
    let summary = Summary::from_report(&report);
    assert_eq!(summary.total, 1);
    assert_eq!(summary.failed, 0);
    assert!(!dir.path().join("song.wav").exists());
}

#[test]
fn library_pipeline_is_silent() {
    if std::env::var_os("FLACSER_SILENCE_CHILD").is_some() {
        external_consumer_can_run_dry_run_pipeline();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "library_pipeline_is_silent", "--nocapture"])
        .env("FLACSER_SILENCE_CHILD", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("processed"));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_preserves_wav_warning_and_progress() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("song.flac");
    std::fs::write(&input, b"audio").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flacser"))
        .args([
            "convert",
            input.to_str().unwrap(),
            "--to",
            "wav",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("[1/1] processed"));
    assert!(String::from_utf8_lossy(&output.stderr).contains(flacser::ffmpeg::WAV_METADATA_NOTE));
}

#[cfg(unix)]
#[test]
fn failed_conversion_is_silent_and_cli_reports_diagnostics() {
    use std::os::unix::fs::PermissionsExt;
    if std::env::var_os("FLACSER_FAILURE_CHILD").is_some() {
        let input = std::path::PathBuf::from(std::env::var_os("FLACSER_INPUT").unwrap());
        let config = Config {
            input_path: input,
            output_dir: None,
            dry_run: false,
            recursive: false,
            flatten: false,
            jobs: 1,
            target_format: AudioFormat::Flac,
        };
        let report = flacser::run_conversion(&config, &InterruptFlag::new()).unwrap();
        let summary = Summary::from_report(&report);
        assert_eq!(summary.failed, 1);
        assert!(summary.failure_details[0].1.contains("fake diagnostic"));
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("song.wav");
    std::fs::write(&input, b"audio").unwrap();
    let ffmpeg = dir.path().join("ffmpeg");
    std::fs::write(&ffmpeg, "#!/bin/sh\nif [ \"$1\" = -version ]; then echo 'ffmpeg version fake'; exit 0; fi\necho fake diagnostic >&2\necho fake stdout\nexit 1\n").unwrap();
    std::fs::set_permissions(&ffmpeg, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "failed_conversion_is_silent_and_cli_reports_diagnostics",
            "--nocapture",
        ])
        .env("FLACSER_FAILURE_CHILD", "1")
        .env("FLACSER_INPUT", &input)
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("fake stdout") && !stdout.contains("processed"));
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_flacser"))
        .args(["convert", input.to_str().unwrap(), "--to", "flac"])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(!cli.status.success());
    assert!(String::from_utf8_lossy(&cli.stderr).contains("fake diagnostic"));
}

#[cfg(unix)]
fn output_with_timeout(command: &mut std::process::Command) -> std::process::Output {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("subprocess timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    std::process::Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

#[cfg(unix)]
#[test]
fn diagnostics_capture_limits_and_streaming() {
    use flacser::convert::JobResult;
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = std::env::var_os("FLACSER_DIAG_MODE") {
        let mode = mode.to_string_lossy();
        let input = std::path::PathBuf::from(std::env::var_os("FLACSER_INPUT").unwrap());
        let config = Config {
            input_path: input,
            output_dir: None,
            dry_run: false,
            recursive: false,
            flatten: false,
            jobs: 1,
            target_format: AudioFormat::Flac,
        };
        let report = flacser::run_conversion(&config, &InterruptFlag::new()).unwrap();
        match &report.results[0] {
            JobResult::Converted { diagnostics } => {
                assert!(mode != "failure" && mode != "publish");
                assert!(diagnostics.len() <= 65536);
                if mode == "exact" {
                    assert_eq!(diagnostics.len(), 65536);
                    assert!(!diagnostics.contains("truncated"));
                }
                if mode == "large" || mode == "invalid" {
                    assert!(diagnostics.contains("truncated"));
                }
                if mode == "small" {
                    assert!(diagnostics.contains("fake diagnostic"));
                }
            }
            JobResult::Failed { error, .. } => {
                assert!(mode == "failure" || mode == "publish", "{error}");
                if mode == "failure" {
                    assert!(error.contains("truncated"));
                }
                if mode == "publish" {
                    assert!(error.contains("fake diagnostic"));
                    assert!(error.contains("failed to publish"));
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
        return;
    }
    for mode in ["small", "exact", "large", "invalid", "failure", "publish"] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("song.wav");
        std::fs::write(&input, b"audio").unwrap();
        let script = dir.path().join("ffmpeg");
        std::fs::write(
            &script,
            r##"#!/bin/sh
if [ "$1" = -version ]; then echo 'ffmpeg version fake'; exit 0; fi
case "$FLACSER_DIAG_MODE" in
exact) head -c 65536 /dev/zero | tr '\000' x >&2 ;;
large|failure) head -c 262144 /dev/zero | tr '\000' x >&2 ;;
invalid) head -c 65536 /dev/zero | tr '\000' '\377' >&2 ;;
*) echo 'fake diagnostic' >&2 ;;
esac
[ "$FLACSER_DIAG_MODE" = failure ] && exit 1
for last do :; done
: > "$last"
[ "$FLACSER_DIAG_MODE" = publish ] && rm "$last"
exit 0
"##,
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::join_paths(std::iter::once(dir.path().to_path_buf()).chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .unwrap();
        let output = output_with_timeout(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "diagnostics_capture_limits_and_streaming",
                    "--nocapture",
                ])
                .env("FLACSER_DIAG_MODE", mode)
                .env("FLACSER_INPUT", &input)
                .env("FLACSER_FINAL", dir.path().join("song.flac"))
                .env("PATH", &path),
        );
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.starts_with("\nrunning 1 test\ntest diagnostics_capture_limits_and_streaming ... ok\n\ntest result: ok."), "{stdout}");
        assert!(
            !stdout.contains("fake diagnostic") && !stdout.contains("processed"),
            "{stdout}"
        );
        if mode == "small" || mode == "failure" {
            let _ = std::fs::remove_file(dir.path().join("song.flac"));
            let cli = output_with_timeout(
                std::process::Command::new(env!("CARGO_BIN_EXE_flacser"))
                    .args(["convert", input.to_str().unwrap(), "--to", "flac"])
                    .env("FLACSER_DIAG_MODE", mode)
                    .env("PATH", &path),
            );
            assert_eq!(cli.status.success(), mode == "small");
            assert!(!cli.stderr.is_empty());
            if mode == "small" {
                assert!(String::from_utf8_lossy(&cli.stderr).contains("fake diagnostic"));
            }
        }
    }
}
