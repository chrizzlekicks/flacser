mod support;

use std::fs;
use tempfile::TempDir;

use assert_cmd::Command;
use support::{FakeFfmpeg, install_fake_ffmpeg, prepend_path, stdout_text, write_file};

#[test]
fn convert_simple_circle_in_same_directory() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: false,
        },
    );
    let path = prepend_path(&bin_dir);

    // First conversion: flac -> aiff
    let assert1 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--to")
        .arg("aiff")
        .env("PATH", &path)
        .assert()
        .success();

    let stdout1 = stdout_text(&assert1);
    assert!(stdout1.contains("total=1"));
    assert!(stdout1.contains("converted=1"));
    assert!(tmp.path().join("song.aiff").exists());

    // Second conversion: aiff -> wav
    let aiff_input = tmp.path().join("song.aiff");
    let assert2 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&aiff_input)
        .arg("--to")
        .arg("wav")
        .env("PATH", &path)
        .assert()
        .success();

    let stdout2 = stdout_text(&assert2);
    assert!(stdout2.contains("total=1"));
    assert!(stdout2.contains("converted=1"));
    assert!(tmp.path().join("song.wav").exists());
}

#[test]
fn convert_circle_with_output_dir_reuse() {
    let tmp = TempDir::new().expect("create temp dir");
    let input_dir = tmp.path().join("input");
    let first_out_dir = tmp.path().join("out");
    let final_out_dir = tmp.path().join("final");
    fs::create_dir_all(&input_dir).expect("create input dir");
    fs::create_dir_all(&first_out_dir).expect("create first out dir");
    fs::create_dir_all(&final_out_dir).expect("create final out dir");

    let input = input_dir.join("album").join("song.flac");
    fs::create_dir_all(input.parent().unwrap()).expect("create album dir");
    write_file(&input);

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

    // First conversion: flac -> aiff with output dir
    let assert1 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input_dir)
        .arg("--to")
        .arg("aiff")
        .arg("--output-dir")
        .arg(&first_out_dir)
        .arg("--recursive") // Need recursive to find files in subdirectories
        .env("PATH", &path)
        .assert()
        .success();

    let stdout1 = stdout_text(&assert1);
    assert!(stdout1.contains("total=1"));
    assert!(stdout1.contains("converted=1"));
    assert!(first_out_dir.join("album").join("song.aiff").exists());

    // Second conversion: aiff -> wav with different output dir
    let aiff_input = first_out_dir.join("album").join("song.aiff");
    let assert2 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&aiff_input)
        .arg("--to")
        .arg("wav")
        .arg("--output-dir")
        .arg(&final_out_dir)
        .env("PATH", &path)
        .assert()
        .success();

    let stdout2 = stdout_text(&assert2);
    assert!(stdout2.contains("total=1"));
    assert!(stdout2.contains("converted=1"));
    // When converting a single file with --output-dir, the file goes directly in the output dir
    assert!(final_out_dir.join("song.wav").exists());
}

#[test]
fn convert_circle_hits_pre_existing_output_skip() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);

    // Pre-create the wav output
    let wav_output = tmp.path().join("song.wav");
    write_file(&wav_output);

    fs::create_dir_all(&bin_dir).expect("create bin dir");

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: false,
        },
    );
    let path = prepend_path(&bin_dir);

    // First conversion: flac -> aiff
    let assert1 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--to")
        .arg("aiff")
        .env("PATH", &path)
        .assert()
        .success();

    let stdout1 = stdout_text(&assert1);
    assert!(stdout1.contains("total=1"));
    assert!(stdout1.contains("converted=1"));
    assert!(tmp.path().join("song.aiff").exists());

    // Second conversion: aiff -> wav (should skip existing)
    let aiff_input = tmp.path().join("song.aiff");
    let assert2 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&aiff_input)
        .arg("--to")
        .arg("wav")
        .env("PATH", &path)
        .assert()
        .success();

    let stdout2 = stdout_text(&assert2);
    assert!(stdout2.contains("total=1"));
    assert!(stdout2.contains("converted=0"));
    assert!(stdout2.contains("skipped=1"));
}

#[test]
fn convert_chain_collapsing_to_source_extension_is_rejected() {
    let tmp = TempDir::new().expect("create temp dir");
    let input = tmp.path().join("song.flac");
    let bin_dir = tmp.path().join("bin");
    write_file(&input);
    fs::create_dir_all(&bin_dir).expect("create bin dir");

    install_fake_ffmpeg(
        &bin_dir,
        FakeFfmpeg::WriteOutput {
            contents: "",
            create_parent: false,
        },
    );
    let path = prepend_path(&bin_dir);

    // First conversion: flac -> aiff
    let assert1 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&input)
        .arg("--to")
        .arg("aiff")
        .env("PATH", &path)
        .assert()
        .success();

    let stdout1 = stdout_text(&assert1);
    assert!(stdout1.contains("total=1"));
    assert!(stdout1.contains("converted=1"));
    assert!(tmp.path().join("song.aiff").exists());

    // Second conversion: aiff -> aiff (should be rejected)
    let aiff_input = tmp.path().join("song.aiff");
    let assert2 = Command::cargo_bin("flacser")
        .expect("build flacser binary")
        .arg("convert")
        .arg(&aiff_input)
        .arg("--to")
        .arg("aiff")
        .env("PATH", &path)
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert2.get_output().stderr).to_string();
    assert!(stderr.contains("same-format conversion is not supported"));
}
