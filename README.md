# flacser

`flacser` is a Rust CLI tool that converts `.flac` files to `.aiff` using `ffmpeg`.

It supports single-file conversion and directory batch conversion with parallel execution.

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
flacser convert [OPTIONS] <INPUT_PATH>
```

`<INPUT_PATH>` can be:

- a single `.flac` file
- a directory (batch mode)

### Options (v0.1)

- `--output-dir <OUTPUT_DIR>`: write outputs into a specific directory
- `--overwrite`: replace existing outputs
- `--dry-run`: plan/execute flow without running `ffmpeg`
- `--recursive`: recurse into subdirectories in directory mode
- `--jobs <JOBS>`: set the number of parallel conversion jobs

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
- Skips outputs that already exist

### Execution and exits

- Runs jobs in parallel using Rayon
- Default jobs: `max(1, cpu_cores - 1)`
- Summary reports actual workers used for the run
- Continues processing when individual jobs fail
- Exits non-zero if any job fails

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

## Testing

Run all tests:

```bash
cargo test
```

Test suite includes:

- unit tests for discover/plan/convert/summary/config logic
- integration tests for CLI behavior and exit codes
- integration tests with mocked `ffmpeg` via `PATH`

## License

MIT
