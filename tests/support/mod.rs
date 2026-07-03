use std::{env, ffi::OsString, fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[allow(dead_code)]
pub enum FakeFfmpeg<'a> {
    VersionOnlySuccess {
        version_line: &'a str,
        extra_version_output: &'a [&'a str],
        non_version_exit: i32,
    },
    VersionOnlyExit {
        version_exit_code: i32,
        non_version_exit: i32,
    },
    ConvertExit {
        code: i32,
    },
    WriteOutput {
        contents: &'a str,
        create_parent: bool,
    },
    WriteArgs {
        create_parent: bool,
    },
    FailOnInputBasename {
        bad_input: &'a str,
        fail_code: i32,
        success_contents: &'a str,
        create_parent: bool,
    },
}

pub fn prepend_path(bin_dir: &Path) -> OsString {
    let old_path = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir.to_path_buf()];
    paths.extend(env::split_paths(&old_path));
    env::join_paths(paths).expect("join PATH entries")
}

pub fn install_fake_ffmpeg(dir: &Path, behavior: FakeFfmpeg<'_>) {
    fs::create_dir_all(dir).expect("create fake ffmpeg dir");

    #[cfg(unix)]
    install_unix_fake_ffmpeg(dir, behavior);

    #[cfg(windows)]
    install_windows_fake_ffmpeg(dir, behavior);
}

#[allow(dead_code)]
pub fn install_fake_ffprobe(dir: &Path, probe_output: &str) {
    fs::create_dir_all(dir).expect("create fake ffprobe dir");

    #[cfg(unix)]
    install_unix_fake_ffprobe(dir, probe_output);

    #[cfg(windows)]
    install_windows_fake_ffprobe(dir, probe_output);
}

#[cfg(unix)]
fn install_unix_fake_ffmpeg(dir: &Path, behavior: FakeFfmpeg<'_>) {
    let ffmpeg_path = dir.join("ffmpeg");
    let script = unix_script(&behavior);
    fs::write(&ffmpeg_path, script).expect("write fake ffmpeg");
    let mut perms = fs::metadata(&ffmpeg_path)
        .expect("stat fake ffmpeg")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&ffmpeg_path, perms).expect("chmod fake ffmpeg");

    install_unix_fake_ffprobe(
        dir,
        "sample_fmt=s16\nbits_per_sample=16\nbits_per_raw_sample=N/A\n",
    );
}

#[cfg(windows)]
fn install_windows_fake_ffmpeg(dir: &Path, behavior: FakeFfmpeg<'_>) {
    let ffmpeg_path = dir.join("ffmpeg.cmd");
    let script = windows_script(&behavior);
    fs::write(&ffmpeg_path, script).expect("write fake ffmpeg");

    install_windows_fake_ffprobe(
        dir,
        "sample_fmt=s16\nbits_per_sample=16\nbits_per_raw_sample=N/A\n",
    );
}

#[cfg(unix)]
fn install_unix_fake_ffprobe(dir: &Path, probe_output: &str) {
    let ffprobe_path = dir.join("ffprobe");
    fs::write(&ffprobe_path, unix_ffprobe_script(probe_output)).expect("write fake ffprobe");
    let mut perms = fs::metadata(&ffprobe_path)
        .expect("stat fake ffprobe")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&ffprobe_path, perms).expect("chmod fake ffprobe");
}

#[cfg(windows)]
fn install_windows_fake_ffprobe(dir: &Path, probe_output: &str) {
    let ffprobe_path = dir.join("ffprobe.cmd");
    fs::write(&ffprobe_path, windows_ffprobe_script(probe_output)).expect("write fake ffprobe");
}

#[cfg(unix)]
fn unix_script(behavior: &FakeFfmpeg<'_>) -> String {
    match behavior {
        FakeFfmpeg::VersionOnlySuccess {
            version_line,
            extra_version_output,
            non_version_exit,
        } => {
            let extra = extra_version_output
                .iter()
                .map(|line| format!("printf '%s\\n' '{}'\n", shell_escape(line)))
                .collect::<String>();
            format!(
                "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' '{}'\n{extra}  exit 0\nfi\nexit {non_version_exit}\n",
                shell_escape(version_line),
            )
        }
        FakeFfmpeg::VersionOnlyExit {
            version_exit_code,
            non_version_exit,
        } => format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  exit {version_exit_code}\nfi\nexit {non_version_exit}\n"
        ),
        FakeFfmpeg::ConvertExit { code } => format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffmpeg version test'\n  exit 0\nfi\nexit {code}\n"
        ),
        FakeFfmpeg::WriteOutput {
            contents,
            create_parent,
        } => format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffmpeg version test'\n  exit 0\nfi\noutput=''\nfor arg in \"$@\"; do output=\"$arg\"; done\n{}{}\nexit 0\n",
            if *create_parent {
                "mkdir -p \"$(dirname \"$output\")\"\n"
            } else {
                ""
            },
            unix_write_output(contents),
        ),
        FakeFfmpeg::WriteArgs { create_parent } => format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffmpeg version test'\n  exit 0\nfi\noutput=''\nfor arg in \"$@\"; do output=\"$arg\"; done\n{}printf '%s' \"$*\" > \"$output\"\nexit 0\n",
            if *create_parent {
                "mkdir -p \"$(dirname \"$output\")\"\n"
            } else {
                ""
            },
        ),
        FakeFfmpeg::FailOnInputBasename {
            bad_input,
            fail_code,
            success_contents,
            create_parent,
        } => format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffmpeg version test'\n  exit 0\nfi\ninput=''\noutput=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-i\" ] && [ $# -ge 2 ]; then\n    input=\"$2\"\n    shift 2\n    continue\n  fi\n  output=\"$1\"\n  shift\ndone\nif [ \"$(basename \"$input\")\" = '{}' ]; then\n  exit {fail_code}\nfi\n{}{}\nexit 0\n",
            shell_escape(bad_input),
            if *create_parent {
                "mkdir -p \"$(dirname \"$output\")\"\n"
            } else {
                ""
            },
            unix_write_output(success_contents),
        ),
    }
}

#[cfg(windows)]
fn windows_script(behavior: &FakeFfmpeg<'_>) -> String {
    match behavior {
        FakeFfmpeg::VersionOnlySuccess {
            version_line,
            extra_version_output,
            non_version_exit,
        } => {
            let extra = extra_version_output
                .iter()
                .map(|line| format!("  echo {}\r\n", windows_echo_escape(line)))
                .collect::<String>();
            format!(
                "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo {}\r\n{extra}  exit /b 0\r\n)\r\nexit /b {non_version_exit}\r\n",
                windows_echo_escape(version_line),
            )
        }
        FakeFfmpeg::VersionOnlyExit {
            version_exit_code,
            non_version_exit,
        } => format!(
            "@echo off\r\nif \"%~1\"==\"-version\" exit /b {version_exit_code}\r\nexit /b {non_version_exit}\r\n"
        ),
        FakeFfmpeg::ConvertExit { code } => format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version test\r\n  exit /b 0\r\n)\r\nexit /b {code}\r\n"
        ),
        FakeFfmpeg::WriteOutput {
            contents,
            create_parent,
        } => format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version test\r\n  exit /b 0\r\n)\r\ncall :parse_args %*\r\n{}{}\r\nexit /b 0\r\n\r\n:parse_args\r\nset \"output=\"\r\n:parse_loop\r\nif \"%~1\"==\"\" goto :eof\r\nset \"output=%~1\"\r\nshift\r\ngoto parse_loop\r\n",
            windows_create_parent(*create_parent),
            windows_write_output(contents),
        ),
        FakeFfmpeg::WriteArgs { create_parent } => format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version test\r\n  exit /b 0\r\n)\r\ncall :parse_args %*\r\n{}> \"%output%\" <nul set /p =%*\r\nexit /b 0\r\n\r\n:parse_args\r\nset \"output=\"\r\n:parse_loop\r\nif \"%~1\"==\"\" goto :eof\r\nset \"output=%~1\"\r\nshift\r\ngoto parse_loop\r\n",
            windows_create_parent(*create_parent),
        ),
        FakeFfmpeg::FailOnInputBasename {
            bad_input,
            fail_code,
            success_contents,
            create_parent,
        } => format!(
            "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffmpeg version test\r\n  exit /b 0\r\n)\r\ncall :parse_args %*\r\nfor %%I in (\"%input%\") do set \"input_name=%%~nxI\"\r\nif \"%input_name%\"==\"{bad_input}\" exit /b {fail_code}\r\n{}{}\r\nexit /b 0\r\n\r\n:parse_args\r\nset \"input=\"\r\nset \"output=\"\r\n:parse_loop\r\nif \"%~1\"==\"\" goto :eof\r\nif \"%~1\"==\"-i\" (\r\n  set \"input=%~2\"\r\n  shift\r\n  shift\r\n  goto parse_loop\r\n)\r\nset \"output=%~1\"\r\nshift\r\ngoto parse_loop\r\n",
            windows_create_parent(*create_parent),
            windows_write_output(success_contents),
        ),
    }
}

#[cfg(unix)]
fn unix_write_output(contents: &str) -> String {
    if contents.is_empty() {
        "touch \"$output\"\n".to_string()
    } else {
        format!("printf '%s' '{}' > \"$output\"\n", shell_escape(contents))
    }
}

#[cfg(unix)]
fn unix_ffprobe_script(probe_output: &str) -> String {
    let output = probe_output
        .lines()
        .map(|line| format!("printf '%s\\n' '{}'\n", shell_escape(line)))
        .collect::<String>();
    format!(
        "#!/bin/sh\nif [ \"${{1:-}}\" = \"-version\" ]; then\n  printf '%s\\n' 'ffprobe version test'\n  exit 0\nfi\n{output}exit 0\n"
    )
}

#[cfg(windows)]
fn windows_ffprobe_script(probe_output: &str) -> String {
    let output = probe_output
        .lines()
        .map(|line| format!("echo {}\r\n", windows_echo_escape(line)))
        .collect::<String>();
    format!(
        "@echo off\r\nif \"%~1\"==\"-version\" (\r\n  echo ffprobe version test\r\n  exit /b 0\r\n)\r\n{output}exit /b 0\r\n"
    )
}

#[cfg(windows)]
fn windows_create_parent(create_parent: bool) -> String {
    if create_parent {
        "for %%I in (\"%output%\") do if not exist \"%%~dpI\" mkdir \"%%~dpI\"\r\n".to_string()
    } else {
        String::new()
    }
}

#[cfg(windows)]
fn windows_write_output(contents: &str) -> String {
    if contents.is_empty() {
        "type nul > \"%output%\"\r\n".to_string()
    } else {
        format!(
            "> \"%output%\" <nul set /p ={}\r\n",
            windows_setp_escape(contents)
        )
    }
}

#[cfg(unix)]
fn shell_escape(text: &str) -> String {
    text.replace('\'', "'\"'\"'")
}

#[cfg(windows)]
fn windows_echo_escape(text: &str) -> String {
    text.replace('^', "^^")
}

#[cfg(windows)]
fn windows_setp_escape(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '^' | '&' | '<' | '>' | '|' => {
                escaped.push('^');
                escaped.push(ch);
            }
            '%' => escaped.push_str("%%"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
