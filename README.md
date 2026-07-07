# flacser

`flacser` is a Rust CLI tool that converts `.flac` files to `.aiff` using `ffmpeg`.

It supports single-file conversion, directory batch conversion with parallel execution, and a read-only `doctor` command for readiness checks.

## Requirements

- `ffmpeg`
- Rust toolchain (for building from source)

Install `ffmpeg`:

```bash
# Arch 
sudo pacman -S ffmpeg

# Ubuntu / Debian
sudo apt install ffmpeg

# macOS (Homebrew)
brew install ffmpeg
```

Windows:

- install FFmpeg
- ensure `ffmpeg.exe` is available on `PATH`

## Build

```bash
cargo build --release
```

Binary path:

```bash
target/release/flacser
```

## Usage

```bash
flacser <COMMAND> [OPTIONS]
```

### Commands

- `convert [OPTIONS] <INPUT_PATH>`: convert a `.flac` file or directory of `.flac` files
- `doctor [OPTIONS] <INPUT_PATH>`: check whether the system is ready to run conversions

`<INPUT_PATH>` for `convert` can be:

- a single `.flac` file
- a directory (batch mode)

### Options

`convert`:

- `--output-dir <OUTPUT_DIR>, -o <OUTPUT_DIR>`: write outputs into a specific directory
- `--dry-run, -n`: plan/execute flow without running `ffmpeg`
- `--recursive, -r`: recurse into subdirectories in directory mode
- `--flatten, -f`: write recursive directory outputs directly into the output directory and fail if flattened output names collide
- `--jobs <JOBS>, -j <JOBS>`: set the number of parallel conversion jobs

`doctor`:

- `--output-dir <OUTPUT_DIR>, -o <OUTPUT_DIR>`: diagnose a specific output directory
- `--jobs <JOBS>, -j <JOBS>`: diagnose a specific parallel worker limit

## Behavior

### File mode

- Converts one `.flac` file to `.aiff`
- Default output path is next to the input file
- If `--output-dir` is set, output is written there

### Directory mode

- Non-recursive by default (top-level only)
- Recurses into subdirectories when `--recursive` is set
- Finds `.flac` files case-insensitively
- Preserves relative structure from the input root
- Non-flatten planning checks collisions on exact output paths
- With `--recursive --flatten`, writes all outputs directly into the output root and fails if flattened output names collide, including ASCII case-insensitive pairs such as `Song.aiff` and `song.aiff`
- `--flatten` has no effect without `--recursive` because non-recursive discovery only sees top-level files
- Skips outputs that already exist

### Execution and exits

- Runs jobs in parallel using Rayon
- Prints per-job progress during execution as `[completed/total] processed`
- Default jobs: `max(1, cpu_cores - 1)`
- Summary reports actual workers used for the run
- Continues processing when individual jobs fail
- Exits non-zero if any job fails

### Doctor command

- Prints a read-only report with `ok`, `warn`, and `fail` checks
- Verifies `ffmpeg` availability and version
- Checks detected CPU cores and default worker settings
- Optionally validates an input path, output directory, and configured worker limit
- Exits non-zero when any required check fails

## Examples

Single file:

```bash
flacser convert ./music/track.flac
```

Single file with output dir:

```bash
flacser convert ./music/track.flac --output-dir ./out
```

Directory dry run:

```bash
flacser convert ./music --dry-run
```

Directory conversion with two parallel jobs:

```bash
flacser convert ./music --jobs 2
```

Flatten recursive directory output:

```bash
flacser convert ./music --recursive --flatten --output-dir ./out
```

System readiness check:

```bash
flacser doctor ./music --output-dir ./out --jobs 2
```

## Testing

Run all tests:

```bash
cargo test
```

Test suite includes:

- unit tests for discover/plan/convert/summary/config logic
- integration tests for CLI behavior and exit codes
- cross-platform integration tests with mocked `ffmpeg` and `PATH` portability helpers

CI runs the Rust test suite on Ubuntu, Windows, and macOS.

Interrupt handling is covered by tests; OS signal hookup remains platform-specific.

## License

MIT
