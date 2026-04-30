use std::{env, fs, path::Path};

use assert_cmd::Command;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn write_file(path: &Path) {
    fs::write(path, b"").expect("write test file");
}

fn stdout_text(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stdout).to_string()
}

fn stderr_text(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stderr).to_string()
}

#[cfg(unix)]
fn prepend_path(bin_dir: &Path) -> std::ffi::OsString {
    let old_path = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.to_path_buf()];
    paths.extend(env::split_paths(&old_path));
    env::join_paths(paths).expect("join PATH entries")
}

#[cfg(unix)]
fn install_fake_ffmpeg_script(dir: &Path, script_body: &str) {
    let ffmpeg_path = dir.join("ffmpeg");
    fs::write(&ffmpeg_path, script_body).expect("write fake ffmpeg");
    let mut perms = fs::metadata(&ffmpeg_path)
        .expect("stat fake ffmpeg")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&ffmpeg_path, perms).expect("chmod fake ffmpeg");
}

#[test]
fn summary_line_is_stable_for_dry_run_success() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("track.flac");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] done\nSummary: total=1, converted=1, skipped=0, failed=0, workers=1\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn summary_line_is_stable_for_skip_case() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("track.flac");
    let output = tmp.path().join("track.aiff");
    write_file(&input);
    write_file(&output);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] done\nSummary: total=1, converted=0, skipped=1, failed=0, workers=1\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn summary_reports_zero_workers_for_empty_input() {
    let tmp = TempDir::new().expect("create temp dir");

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(tmp.path())
        .arg("--dry-run")
        .arg("--jobs")
        .arg("8")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "Summary: total=0, converted=0, skipped=0, failed=0, workers=0\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn summary_reports_actual_workers_used() {
    let tmp = TempDir::new().expect("create temp dir");
    let first = tmp.path().join("first.flac");
    let second = tmp.path().join("second.flac");
    write_file(&first);
    write_file(&second);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(tmp.path())
        .arg("--dry-run")
        .arg("--jobs")
        .arg("8")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/2] done\n[2/2] done\nSummary: total=2, converted=2, skipped=0, failed=0, workers=2\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn failure_prints_summary_to_stdout_and_details_to_stderr() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("broken.flac");
    write_file(&input);

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    #[cfg(unix)]
    install_fake_ffmpeg_script(&bin_dir, "#!/bin/sh\nexit 11\n");
    #[cfg(unix)]
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] done\nSummary: total=1, converted=0, skipped=0, failed=1, workers=1\n"
    );
    assert!(stderr.contains("FAILED"));
    assert!(stderr.contains(&input.display().to_string()));
    assert!(stderr.contains("ffmpeg exited with status"));
}

#[test]
fn partial_batch_failure_has_predictable_summary_counts() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("input");
    let output_dir = tmp.path().join("out");
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&input_dir).expect("create input dir");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    let ok = input_dir.join("ok.flac");
    let bad = input_dir.join("bad.flac");
    write_file(&ok);
    write_file(&bad);

    #[cfg(unix)]
    install_fake_ffmpeg_script(
        &bin_dir,
        "#!/bin/sh\ninput=\"\"\noutput=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-i\" ] && [ $# -ge 2 ]; then\n    input=\"$2\"\n    shift 2\n    continue\n  fi\n  output=\"$1\"\n  shift\ndone\nif [ \"$(basename \"$input\")\" = \"bad.flac\" ]; then\n  exit 19\nfi\nmkdir -p \"$(dirname \"$output\")\"\ntouch \"$output\"\nexit 0\n",
    );
    #[cfg(unix)]
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert!(stdout.contains("[1/2] done\n"));
    assert!(stdout.contains("[2/2] done\n"));
    assert!(stdout.ends_with("Summary: total=2, converted=1, skipped=0, failed=1, workers=2\n"));
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("FAILED "))
            .count(),
        1
    );
}
