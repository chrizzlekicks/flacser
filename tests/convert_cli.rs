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

fn output_text(assert: &assert_cmd::assert::Assert) -> String {
    format!("{}{}", stdout_text(assert), stderr_text(assert))
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
fn convert_missing_path_exits_non_zero() {
    let tmp = TempDir::new().expect("create temp dir");
    let missing = tmp.path().join("missing.flac");

    Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&missing)
        .assert()
        .failure();
}

#[test]
fn convert_single_file_dry_run_succeeds_without_ffmpeg() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
}

#[test]
fn flacser_without_subcommand_shows_usage_and_fails() {
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .assert()
        .failure();

    let output = output_text(&assert);
    assert!(output.contains("Usage: flacser <COMMAND>"));
    assert!(output.contains("convert"));
}

#[test]
fn convert_help_shows_expected_contract() {
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg("--help")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("Usage: flacser convert [OPTIONS] <INPUT_PATH>"));
    assert!(stdout.contains("--output-dir <OUTPUT_DIR>"));
    assert!(stdout.contains("--overwrite"));
    assert!(stdout.contains("--dry-run"));
    assert!(stdout.contains("--recursive"));
    assert!(stdout.contains("--jobs <JOBS>"));
}

#[test]
fn convert_rejects_zero_jobs() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--jobs")
        .arg("0")
        .assert()
        .failure();

    let stderr = stderr_text(&assert);
    assert!(stderr.contains("--jobs <JOBS>"));
}

#[test]
fn convert_non_flac_input_file_exits_non_zero_with_clear_error() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.wav");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .assert()
        .failure();

    let stderr = stderr_text(&assert);
    assert!(stderr.contains("input file is not a .flac file"));
}

#[test]
fn convert_directory_is_non_recursive_by_default() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();
    let top_level = root.join("top.flac");
    let nested_dir = root.join("nested");
    let nested_flac = nested_dir.join("inner.flac");
    write_file(&top_level);
    fs::create_dir_all(&nested_dir).expect("create nested dir");
    write_file(&nested_flac);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(root)
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("skipped=0"));
}

#[test]
fn convert_directory_recurses_when_recursive_is_enabled() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();
    let top_level = root.join("top.flac");
    let nested_dir = root.join("nested");
    let nested_flac = nested_dir.join("inner.flac");
    write_file(&top_level);
    fs::create_dir_all(&nested_dir).expect("create nested dir");
    write_file(&nested_flac);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(root)
        .arg("--dry-run")
        .arg("--recursive")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=2"));
    assert!(stdout.contains("converted=2"));
    assert!(stdout.contains("skipped=0"));
}

#[test]
fn convert_directory_supports_case_insensitive_flac_extension() {
    let tmp = TempDir::new().expect("create temp dir");
    let root = tmp.path();
    let upper = root.join("LOUD.FLAC");
    write_file(&upper);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(root)
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
}

#[test]
fn convert_empty_directory_succeeds_with_zero_summary() {
    let tmp = TempDir::new().expect("create temp dir");

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(tmp.path())
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=0"));
    assert!(stdout.contains("converted=0"));
    assert!(stdout.contains("failed=0"));
}

#[test]
fn convert_skips_existing_output() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let output = tmp.path().join("song.aiff");
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
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=0"));
    assert!(stdout.contains("skipped=1"));
    assert!(stdout.contains("failed=0"));
}

#[test]
fn convert_overwrite_converts_existing_output() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let output = tmp.path().join("song.aiff");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::write(&output, b"old").expect("write existing output");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    #[cfg(unix)]
    install_fake_ffmpeg_script(
        &bin_dir,
        "#!/bin/sh\noutput=\"\"\nfor arg in \"$@\"; do output=\"$arg\"; done\nprintf new > \"$output\"\nexit 0\n",
    );

    #[cfg(unix)]
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--overwrite")
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("skipped=0"));
    assert!(stdout.contains("failed=0"));
    assert_eq!(fs::read(&output).expect("read output"), b"new");
}

#[test]
fn convert_invalid_output_dir_path_exits_non_zero() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let output_as_file = tmp.path().join("not-a-directory");
    write_file(&input);
    write_file(&output_as_file);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--output-dir")
        .arg(&output_as_file)
        .assert()
        .failure();

    let stderr = stderr_text(&assert);
    assert!(stderr.contains("output path exists but is not a directory"));
}

#[test]
fn convert_single_file_writes_to_output_dir_with_mocked_ffmpeg() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let out_dir = tmp.path().join("out");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    #[cfg(unix)]
    install_fake_ffmpeg_script(
        &bin_dir,
        "#!/bin/sh\noutput=\"\"\nfor arg in \"$@\"; do output=\"$arg\"; done\nmkdir -p \"$(dirname \"$output\")\"\ntouch \"$output\"\nexit 0\n",
    );

    #[cfg(unix)]
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--output-dir")
        .arg(&out_dir)
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
    assert!(out_dir.join("song.aiff").exists());
}

#[test]
fn convert_returns_non_zero_when_ffmpeg_fails() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    write_file(&input);

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    #[cfg(unix)]
    install_fake_ffmpeg_script(&bin_dir, "#!/bin/sh\nexit 7\n");
    #[cfg(unix)]
    let new_path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .env("PATH", new_path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);
    assert!(stdout.contains("failed=1"));
    assert!(stderr.contains("FAILED"));
}

#[test]
fn convert_continues_after_failure_and_reports_partial_batch_failure() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("input");
    let output_dir = tmp.path().join("out");
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&input_dir).expect("create input dir");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    let good = input_dir.join("good.flac");
    let bad = input_dir.join("bad.flac");
    write_file(&good);
    write_file(&bad);

    #[cfg(unix)]
    install_fake_ffmpeg_script(
        &bin_dir,
        "#!/bin/sh\ninput=\"\"\noutput=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-i\" ] && [ $# -ge 2 ]; then\n    input=\"$2\"\n    shift 2\n    continue\n  fi\n  output=\"$1\"\n  shift\ndone\nif [ \"$(basename \"$input\")\" = \"bad.flac\" ]; then\n  exit 9\nfi\nmkdir -p \"$(dirname \"$output\")\"\ntouch \"$output\"\nexit 0\n",
    );
    #[cfg(unix)]
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert!(stdout.contains("total=2"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=1"));
    assert!(stderr.contains("FAILED"));
    assert!(output_dir.join("good.aiff").exists());
    assert!(!output_dir.join("bad.aiff").exists());
}
