# flacser

A simple, robust shell script to convert `.flac` audio files to `.aiff` using `ffmpeg`.

Designed with reliability and automation in mind, this script includes input validation, safe execution defaults, and skip logic to avoid redundant work.

---

## ✨ Features

* 🎧 Converts `.flac` → `.aiff`
* 📁 Automatically creates target directory if missing
* ⏭️ Skips files that are already converted
* ❌ Fails fast on invalid input
* 🔒 Safe execution with `set -euo pipefail`
* 🤖 Designed to be scriptable and testable

---

## 📦 Requirements

* `zsh`
* `ffmpeg`

### Install dependencies (examples)

**Ubuntu / Debian:**

```bash
sudo apt install zsh ffmpeg
```

**macOS (Homebrew):**

```bash
brew install zsh ffmpeg
```

---

## 🚀 Usage

```bash
./flac2aiff <path_to_flac>
```

### Example

```bash
./flac2aiff ~/music/track.flac
```

---

## ⚙️ Configuration

By default, converted files are written to:

```bash
/home/chrizzle/music/source_aiff
```

### Override target directory

You can override the output directory via environment variable:

```bash
TARGET_DIR=/custom/output/path ./flac2aiff track.flac
```

---

## 📁 Output Behavior

Given:

```bash
input:  /music/song.flac
output: $TARGET_DIR/song.aiff
```

---

## 🔁 Skip Logic

If the output file already exists:

```bash
⏭️  Skipping: song.aiff already exists.
```

The script exits successfully without reprocessing.

---

## ❌ Error Handling

### Missing argument

```bash
Usage: ./flac2aiff <path_to_flac>
```

### Input file not found

```bash
❌ Error: File '...' not found.
```

### Invalid target path

```bash
❌ Error: Target path '...' exists but is NOT a directory.
```

---

## 🧠 Implementation Details

### Safe Shell Practices

The script uses:

```bash
set -euo pipefail
```

* `-e`: exit on error
* `-u`: fail on unset variables
* `-o pipefail`: catch pipeline errors

### Argument Safety

Uses:

```bash
${1:-}
```

or:

```bash
(( $# == 0 ))
```

to avoid crashes when no arguments are passed.

---

## 🔊 Conversion Command

Internally uses:

```bash
ffmpeg -nostdin -i "$input_file" \
       -map 0 -write_id3v2 1 -y \
       -loglevel error \
       "$output_path" < /dev/null
```

### Why these flags?

* `-nostdin`: prevents blocking in pipelines
* `-map 0`: include all streams
* `-write_id3v2 1`: preserve metadata
* `-y`: overwrite without prompting
* `< /dev/null`: ensures non-interactive behavior

---

## 🧪 Testing

The script is designed to be tested with a framework like **Bats**.

Key testing strategies:

* Mock `ffmpeg`
* Isolate filesystem using temp directories
* Inject `TARGET_DIR` via environment

---

## 📌 Design Philosophy

This script follows classic Unix principles:

* Do one thing well
* Fail loudly and early
* Avoid hidden side effects
* Be composable in pipelines

---

## 🚧 Possible Improvements

* Batch processing (multiple files / directories)
* Parallel execution
* Logging verbosity levels
* Support for additional formats
* CLI flags (e.g. `--output-dir`, `--force`)

---

## 📜 License

MIT (or your preferred license)

---

## 🙌 Acknowledgements

Built for efficient local audio workflows and automation-friendly pipelines.

