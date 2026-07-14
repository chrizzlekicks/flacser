mod support;

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

use support::{
    FakeFfmpeg, install_fake_ffmpeg, prepend_path, stderr_text, stdout_text, write_file,
};

#[test]
fn summary_line_is_stable_for_dry_run_success() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("track.flac");
    write_file(&input);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--to")
        .arg("aiff")
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] processed\nSummary: total=1, converted=1, skipped=0, failed=0, workers=1\n"
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
        .arg("--to")
        .arg("aiff")
        .arg("--dry-run")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] processed\nSummary: total=1, converted=0, skipped=1, failed=0, workers=1\n"
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
        .arg("--to")
        .arg("aiff")
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
        .arg("--to")
        .arg("aiff")
        .arg("--dry-run")
        .arg("--jobs")
        .arg("8")
        .assert()
        .success();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/2] processed\n[2/2] processed\nSummary: total=2, converted=2, skipped=0, failed=0, workers=2\n"
    );
    assert!(stderr.is_empty());
}

#[test]
fn failure_prints_summary_to_stdout_and_details_to_stderr() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("broken.flac");
    write_file(&input);

    let bin_dir = tmp.path().join("bin");
    install_fake_ffmpeg(&bin_dir, FakeFfmpeg::ConvertExit { code: 11 });
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--to")
        .arg("aiff")
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert_eq!(
        stdout,
        "[1/1] processed\nSummary: total=1, converted=0, skipped=0, failed=1, workers=1\n"
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

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::FailOnInputBasename {
            bad_input: "bad.flac",
            fail_code: 19,
            success_contents: "",
            create_parent: true,
        },
    );
    let path = prepend_path(&bin_dir);

    let assert = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--to")
        .arg("aiff")
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--jobs")
        .arg("2")
        .env("PATH", path)
        .assert()
        .failure();

    let stdout = stdout_text(&assert);
    let stderr = stderr_text(&assert);

    assert!(stdout.contains("[1/2] processed\n"));
    assert!(stdout.contains("[2/2] processed\n"));
    assert!(stdout.ends_with("Summary: total=2, converted=1, skipped=0, failed=1, workers=2\n"));
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("FAILED "))
            .count(),
        1
    );
}
