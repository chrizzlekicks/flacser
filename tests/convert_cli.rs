mod support;

use std::{fs, path::Path};

#[cfg(unix)]
use std::{
    process::{Command as StdCommand, Stdio},
    thread,
    time::Duration,
};

use assert_cmd::Command;
use tempfile::TempDir;

use support::{FakeFfmpeg, install_fake_ffmpeg, prepend_path};

#[cfg(unix)]
const INTERRUPT_FAKE_FFMPEG_START_TIMEOUT: Duration = Duration::from_secs(5);

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

fn assert_usage(output: &str, usage_suffix: &str) {
    let without_extension = format!("Usage: flacser {usage_suffix}");
    let with_extension = format!("Usage: flacser.exe {usage_suffix}");
    assert!(
        output.contains(&without_extension) || output.contains(&with_extension),
        "expected usage line {without_extension:?} or {with_extension:?}\noutput:\n{output}"
    );
}

#[cfg(unix)]
fn install_interrupt_fake_ffmpeg(dir: &Path, marker: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(dir).expect("create fake ffmpeg dir");
    let ffmpeg_path = dir.join("ffmpeg");
    let script = format!(
        "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffmpeg version test'\n  exit 0\nfi\noutput=\"\"\nfor arg in \"$@\"; do output=\"$arg\"; done\nprintf partial > \"$output\"\ntouch \"{}\"\nsleep 1\nexit 9\n",
        marker.display()
    );
    fs::write(&ffmpeg_path, script).expect("write fake ffmpeg");
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
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--dry-run")
        .env("PATH", &bin_dir)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[1/1] processed"));
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
}

#[test]
fn convert_missing_ffmpeg_exits_non_zero_with_install_instructions() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .env("PATH", &bin_dir)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);
    assert!(!stdout.contains("Summary:"));
    assert!(stderr.contains("ffmpeg not found."));
    assert!(stderr.contains("Install it with:"));
    assert!(stderr.contains("Arch:   sudo pacman -S ffmpeg"));
    assert!(stderr.contains("Ubuntu: sudo apt install ffmpeg"));
    assert!(stderr.contains("macOS:  brew install ffmpeg"));
}

#[test]
fn flacser_without_subcommand_shows_usage_and_fails() {
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .assert()
        .failure();

    let output = output_text(&assert);
    assert_usage(&output, "<COMMAND>");
    assert!(output.contains("convert"));
    assert!(output.contains("doctor"));
}

#[test]
fn flacser_help_shows_basic_contract() {
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("--help")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert_usage(&stdout, "<COMMAND>");
    assert!(stdout.contains("convert"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("Convert .flac file or directory with multiple .flac files to .aiff"));
    assert!(stdout.contains("Check whether the system is ready to run conversions"));
    assert!(stdout.contains("help"));
    assert!(stdout.contains("Print this message or the help of the given subcommand(s)"));
    assert!(stdout.contains("-h, --help"));
    assert!(stdout.contains("Print help"));
    assert!(stdout.contains("-V, --version"));
    assert!(stdout.contains("Print version"));
}

#[test]
fn doctor_help_shows_expected_contract() {
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg("--help")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert_usage(&stdout, "doctor [OPTIONS] [INPUT_PATH]");
    assert!(stdout.contains("Optional input path to diagnose before conversion"));
    assert!(stdout.contains("-o, --output-dir <OUTPUT_DIR>"));
    assert!(stdout.contains("-j, --jobs <JOBS>"));
}

#[test]
fn doctor_succeeds_when_global_checks_pass() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version 7.1-test",
            extra_version_output: &["configuration: test"],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("Doctor report:"));
    assert!(stdout.contains("[ok] ffmpeg: found"));
    assert!(stdout.contains("[ok] ffmpeg version: ffmpeg version 7.1-test"));
    assert!(stdout.contains("[ok] cpu cores:"));
    assert!(stdout.contains("[ok] default workers:"));
    assert!(stdout.contains("[ok] config sanity: global defaults are sane"));
    assert!(stdout.contains("Read-only: no files were created, modified, or converted."));
    assert!(stdout.contains("Warnings: no"));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_jobs_warning_still_succeeds() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version 7.1-test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg("--jobs")
        .arg("9999")
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] configured workers: 9999"));
    assert!(stdout.contains("[warn] worker oversubscription:"));
    assert!(stdout.contains("Warnings: yes"));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_file_input_with_jobs_succeeds_with_fake_ffmpeg() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(&input)
        .arg("--jobs")
        .arg("1")
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] input type: file"));
    assert!(stdout.contains("[ok] discoverable files: 1 .flac file(s) found"));
    assert!(stdout.contains("[ok] output planning: 1 output path(s) validated"));
    assert!(!stdout.contains("song.aiff"));
    assert!(stdout.contains("[ok] effective workers: 1"));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_file_input_with_output_dir_succeeds_with_fake_ffmpeg() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let out_dir = tmp.path().join("out");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(&input)
        .arg("--output-dir")
        .arg(&out_dir)
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] output directory: createable under existing parent:"));
    assert!(stdout.contains("[ok] output planning: 1 output path(s) validated"));
    assert!(!stdout.contains(&out_dir.join("song.aiff").display().to_string()));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_directory_input_summarizes_planning_without_output_names() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    let first = tmp.path().join("first.flac");
    let second = tmp.path().join("second.flac");
    write_file(&first);
    write_file(&second);
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(tmp.path())
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] input type: directory"));
    assert!(stdout.contains("[ok] discoverable files: 2 .flac file(s) found"));
    assert!(stdout.contains("[ok] output planning: 2 output path(s) validated"));
    assert!(!stdout.contains("first.aiff"));
    assert!(!stdout.contains("second.aiff"));
    assert!(stdout.contains("[ok] effective workers: 2"));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_output_dir_without_input_runs_output_diagnostics() {
    let tmp = TempDir::new().expect("create temp dir");
    let out_dir = tmp.path().join("out");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg("--output-dir")
        .arg(&out_dir)
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] output directory: createable under existing parent:"));
    assert!(!stdout.contains("input type"));
    assert!(stdout.contains("Ready: yes"));
}

#[test]
fn doctor_empty_directory_exits_non_zero() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(tmp.path())
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[fail] discoverable files: 0 .flac files found"));
    assert!(stdout.contains("Ready: no"));
}

#[test]
fn doctor_missing_input_exits_non_zero() {
    let tmp = TempDir::new().expect("create temp dir");
    let missing = tmp.path().join("missing");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(&missing)
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[fail] input exists: not found or not accessible"));
    assert!(stdout.contains("Ready: no"));
}

#[test]
fn doctor_output_path_file_exits_non_zero() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let output_as_file = tmp.path().join("not-a-dir");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    write_file(&output_as_file);
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(&input)
        .arg("--output-dir")
        .arg(&output_as_file)
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[fail] output directory: exists but is not a directory"));
    assert!(stdout.contains("[fail] output planning: output path exists but is not a directory"));
    assert!(stdout.contains("Ready: no"));
}

#[test]
fn doctor_fails_when_ffmpeg_is_missing() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .env("PATH", &bin_dir)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("Doctor report:"));
    assert!(stdout.contains("[fail] ffmpeg:"));
    assert!(stdout.contains("[fail] ffmpeg version:"));
    assert!(stdout.contains("[ok] cpu cores:"));
    assert!(stdout.contains("[ok] default workers:"));
    assert!(stdout.contains("Ready: no"));
}

#[test]
fn doctor_fails_when_ffmpeg_version_is_unreadable() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlyExit {
            version_exit_code: 42,
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] ffmpeg: found"));
    assert!(stdout.contains("ffmpeg -version exited with status"));
    assert!(stdout.contains("[fail] ffmpeg version:"));
    assert!(stdout.contains("Ready: no"));
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
    assert_usage(&stdout, "convert [OPTIONS] <INPUT_PATH>");
    assert!(stdout.contains("Input `.flac` file or directory to convert"));
    assert!(stdout.contains("-o, --output-dir <OUTPUT_DIR>"));
    assert!(stdout.contains("Write converted `.aiff` files into this directory"));
    assert!(stdout.contains("-n, --dry-run"));
    assert!(stdout.contains("Print the conversion plan without running `ffmpeg`"));
    assert!(stdout.contains("-r, --recursive"));
    assert!(stdout.contains("Recurse into subdirectories when the input path is a directory"));
    assert!(stdout.contains("-j, --jobs <JOBS>"));
    assert!(stdout.contains("Limit the number of parallel conversion jobs"));
    assert!(stdout.contains("-f, --flatten"));
    assert!(stdout.contains("Write all converted files directly into the output directory"));
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
    assert!(stdout.contains("[1/2] processed"));
    assert!(stdout.contains("[2/2] processed"));
    assert!(stdout.contains("total=2"));
    assert!(stdout.contains("converted=2"));
    assert!(stdout.contains("skipped=0"));
}

#[test]
fn convert_directory_recurses_when_recursive_short_flag_is_enabled() {
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
        .arg("-r")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[1/2] processed"));
    assert!(stdout.contains("[2/2] processed"));
    assert!(stdout.contains("total=2"));
    assert!(stdout.contains("converted=2"));
    assert!(stdout.contains("skipped=0"));
}

#[test]
fn convert_accepts_jobs_short_flag() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--dry-run")
        .arg("-j")
        .arg("1")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[1/1] processed"));
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
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
    assert!(!stdout.contains(" processed\n"));
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
    assert!(stdout.contains("[1/1] processed"));
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=0"));
    assert!(stdout.contains("skipped=1"));
    assert!(stdout.contains("failed=0"));
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

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: true,
        },
    );

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
fn convert_handles_paths_with_spaces() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("input files");
    let output_dir = tmp.path().join("converted files");
    let bin_dir = tmp.path().join("fake bin");
    let input = input_dir.join("space song.flac");
    fs::create_dir_all(&input_dir).expect("create input dir");
    fs::create_dir_all(&bin_dir).expect("create bin dir");
    write_file(&input);

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: true,
        },
    );

    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--output-dir")
        .arg(&output_dir)
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=1"));
    assert!(stdout.contains("converted=1"));
    assert!(stdout.contains("failed=0"));
    assert!(output_dir.join("space song.aiff").exists());
}

#[test]
fn convert_returns_non_zero_when_ffmpeg_fails() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    write_file(&input);

    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(&bin_dir, FakeFfmpeg::ConvertExit { code: 7 });
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

#[cfg(unix)]
#[test]
fn convert_interrupt_exits_130_and_removes_partial_temp_output() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    let started = tmp.path().join("started");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    install_interrupt_fake_ffmpeg(&bin_dir, &started);
    let path = prepend_path(&bin_dir);

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("flacser"))
        .arg("convert")
        .arg(&input)
        .env("PATH", path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn flacser");

    let start_wait_deadline = std::time::Instant::now() + INTERRUPT_FAKE_FFMPEG_START_TIMEOUT;
    while std::time::Instant::now() < start_wait_deadline {
        if started.exists() {
            break;
        }
        if let Some(status) = child.try_wait().expect("poll flacser child") {
            let output = child.wait_with_output().expect("collect flacser output");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "flacser exited before fake ffmpeg started with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(started.exists(), "fake ffmpeg should have started");

    let status = StdCommand::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()
        .expect("send SIGINT");
    assert!(status.success());

    let output = child.wait_with_output().expect("wait for flacser");

    assert_eq!(output.status.code(), Some(130));
    assert!(!tmp.path().join("song.aiff").exists());
    let temp_files: Vec<_> = fs::read_dir(tmp.path())
        .expect("read temp dir")
        .map(|entry| entry.expect("read entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".flacser-"))
        })
        .collect();
    assert!(
        temp_files.is_empty(),
        "temp files left behind: {temp_files:?}"
    );
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

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::FailOnInputBasename {
            bad_input: "bad.flac",
            fail_code: 9,
            success_contents: "",
            create_parent: true,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("1")
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert!(
        stdout.contains("total=2"),
        "stdout should contain total=2\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("converted=1"),
        "stdout should contain converted=1\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("failed=1"),
        "stdout should contain failed=1\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("FAILED"),
        "stderr should contain FAILED\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(output_dir.join("good.aiff").exists());
    assert!(!output_dir.join("bad.aiff").exists());
}

#[test]
fn convert_directory_flattens_output_with_flatten() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("music");
    let out_dir = tmp.path().join("aiff");
    let album_a = input_dir.join("album-a");
    let album_b = input_dir.join("album-b");
    fs::create_dir_all(&album_a).expect("create album-a");
    fs::create_dir_all(&album_b).expect("create album-b");
    let song_a = album_a.join("song.flac");
    let track_b = album_b.join("track.flac");
    write_file(&song_a);
    write_file(&track_b);

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: true,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--recursive")
        .arg("--flatten")
        .arg("--output-dir")
        .arg(&out_dir)
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("total=2"));
    assert!(stdout.contains("converted=2"));
    assert!(stdout.contains("failed=0"));
    assert!(out_dir.join("song.aiff").exists());
    assert!(out_dir.join("track.aiff").exists());
}

#[test]
fn convert_flatten_fails_on_basename_collision() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("music");
    let album_a = input_dir.join("album-a");
    let album_b = input_dir.join("album-b");
    fs::create_dir_all(&album_a).expect("create album-a");
    fs::create_dir_all(&album_b).expect("create album-b");
    let song_a = album_a.join("song.flac");
    let song_b = album_b.join("song.flac");
    write_file(&song_a);
    write_file(&song_b);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--recursive")
        .arg("--flatten")
        .arg("--dry-run")
        .assert()
        .failure();

    let stderr = stderr_text(&assert);
    assert!(stderr.contains("output collision detected"));
}

#[test]
fn doctor_directory_with_only_nested_flacs_succeeds() {
    let tmp = TempDir::new().expect("create temp dir");
    let bin_dir = tmp.path().join("bin");
    let nested = tmp.path().join("nested");
    let nested_flac = nested.join("song.flac");
    fs::create_dir_all(&nested).expect("create nested dir");
    write_file(&nested_flac);
    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::VersionOnlySuccess {
            version_line: "ffmpeg version test",
            extra_version_output: &[],
            non_version_exit: 9,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("doctor")
        .arg(tmp.path())
        .env("PATH", path)
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    assert!(stdout.contains("[ok] discoverable files: 1 .flac file(s) found"));
    assert!(stdout.contains("Ready: yes"));
}
