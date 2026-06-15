# flacser

`flacser` is a Rust CLI tool that converts lossless FLAC, AIFF, and WAV files using `ffmpeg`.

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

- `convert [OPTIONS] <INPUT_PATH> --to <FORMAT>`: convert FLAC, AIFF, or WAV files
- `doctor [OPTIONS] [INPUT_PATH]`: check whether the system is ready to run conversions

`<INPUT_PATH>` for `convert` can be:

- a single `.flac`, `.aiff`, or `.wav` file
- a directory (batch mode)

### Options

`convert`:

- `--to <FORMAT>`: target format (`flac`, `aiff`, or `wav`); falls back to `FLACSER_CONVERT_TO`
- `--output-dir <OUTPUT_DIR>, -o <OUTPUT_DIR>`: write outputs into a specific directory
- `--overwrite, -w`: replace existing outputs
- `--dry-run, -n`: plan/execute flow without running `ffmpeg`
- `--recursive, -r`: recurse into subdirectories in directory mode
- `--jobs <JOBS>, -j <JOBS>`: set the number of parallel conversion jobs

`doctor`:

- `--output-dir <OUTPUT_DIR>, -o <OUTPUT_DIR>`: diagnose a specific output directory
- `--jobs <JOBS>, -j <JOBS>`: diagnose a specific parallel worker limit

## Behavior

### File mode

- Converts one supported `.flac`, `.aiff`, or `.wav` input file to the requested target format
- Source format is inferred from the file extension case-insensitively
- Default output path is next to the input file
- If `--output-dir` is set, output is written there
- Same-format conversion is rejected

### Directory mode

- Non-recursive by default (top-level only)
- Recurses into subdirectories when `--recursive` is set
- Finds `.flac`, `.aiff`, and `.wav` files case-insensitively
- Preserves relative structure from the input root
- Uses the target format's canonical extension; AIFF outputs use `.aiff`
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
flacser convert ./music/track.flac --to aiff
```

Single file with output dir:

```bash
flacser convert ./music/track.flac --to wav --output-dir ./out
```

Directory dry run:

```bash
flacser convert ./music --to aiff --dry-run
```

Directory conversion with two parallel jobs:

```bash
flacser convert ./music --to flac --jobs 2
```

Environment fallback target:

```bash
FLACSER_CONVERT_TO=aiff flacser convert ./music
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
