# AGENTS.md

## Project overview

This repository contains **flacser**, a Rust CLI tool for converting `.flac` audio files to `.aiff` using `ffmpeg`.

The tool is designed as a **batch-first, parallel conversion CLI**, supporting both single-file and directory-based workflows.

Core capabilities:

* Convert a single `.flac` file
* Convert all `.flac` files in a directory
* Run conversions in parallel using a bounded worker pool
* Preserve predictable and safe filesystem behavior
* Provide clear output and summaries

`ffmpeg` is used as the backend for all audio processing.

---

## Product philosophy

flacser should be:

* Small and focused
* Predictable and safe
* Fast through parallelism
* Easy to extend without overengineering

This is a **thin orchestration layer over ffmpeg**, not a media processing engine.

---

## CLI design

Subcommands are based on **input type**, not action:

```bash
flacser file <input.flac>
flacser dir <input-dir>
```

### Shared flags

* `-o, --output-dir <DIR>`
* `-j, --jobs <N>` (max parallel conversions)
* `--overwrite`
* `--dry-run`
* `-v, --verbose`

### Directory-specific flags

* `-r, --recursive`
* `--flatten` (optional, default = false)
* `--fail-fast` (optional, default = false)

---

## Default behavior

Unless explicitly overridden:

### File mode

* Converts one file
* Outputs directly into target directory

### Directory mode

* Scans for `.flac` files (non-recursive by default)
* Preserves relative directory structure in output
* Skips files whose output already exists
* Continues processing even if some conversions fail
* Prints a summary at the end
* Returns non-zero exit code if any file failed

---

## Output directory resolution

Priority order:

1. CLI flag `--output-dir`
2. Environment variable `FLACSER_OUTPUT_DIR`
3. Built-in default (must NOT hardcode user-specific absolute paths)

---

## Parallelism model

flacser uses **bounded parallelism via a thread pool**.

### Requirements

* Use **Rayon** for parallel execution
* Do NOT spawn one thread per file
* Number of workers is:

```
min(job_count, configured_jobs)
```

* Default `jobs` = number of available CPU cores (or cores - 1)

### Rationale

* Each job runs an external `ffmpeg` process
* Unbounded concurrency would overwhelm CPU and I/O
* Predictable resource usage is required

---

## Tokio policy

Do NOT introduce Tokio.

Reasons:

* Workload is not async I/O bound
* Jobs are blocking (external processes)
* Rayon is the correct abstraction for data-parallel workloads

If async is introduced in the future, it must be justified explicitly.

---

## Architecture principles

Keep the architecture straightforward and testable.

### Pipeline model

The program should follow a clear pipeline:

1. **Discover**

   * Collect `.flac` files from input

2. **Plan**

   * Map each input → output path
   * Validate paths
   * Detect collisions early

3. **Execute**

   * Run jobs in parallel (Rayon)
   * Each job is independent

4. **Collect**

   * Record per-job results

5. **Summarize**

   * Print final report
   * Set exit code

### Module structure

```
cli.rs        -> CLI definitions (clap)
discover.rs   -> finding input files
plan.rs       -> mapping inputs to outputs
convert.rs    -> executing conversions (Rayon)
ffmpeg.rs     -> process spawning
summary.rs    -> result aggregation
config.rs     -> resolving defaults/env/flags
main.rs       -> entry point and orchestration
```

### Design guidelines

* Keep CLI parsing separate from core logic
* Keep filesystem planning separate from process execution
* Keep result aggregation separate from conversion logic
* Prefer small, composable modules over heavy abstractions
* Optimize for testability

---

## Coding style and conventions

Prefer:

* Simplicity over complexity
* Readability over cleverness
* Pragmatic Rust best practices
* Explicit, understandable control flow
* Small functions with clear responsibilities
* Test-driven development is the priority.
* Spec-driven development when a project starts, and when requirements are unclear, vague, or ambiguous

Write idiomatic Rust when it improves clarity, but avoid advanced idioms or abstractions that reduce readability.

Avoid:

* Overengineering
* Premature abstraction
* Clever one-liners that obscure intent
* Unnecessary generics, traits, or lifetimes
* Mixing business logic with CLI output

---

## Agent workflow

Default workflow for changes:

1. Inspect relevant files
2. Propose a concise implementation plan
3. Ask for approval before editing files
4. After approval, implement only the agreed scope
5. If scope changes, pause and ask again

Do not begin editing immediately unless explicitly instructed.

Once a plan is approved, edits within that scope may proceed without repeated confirmation.

---

## Change discipline

When making changes:

* Make the smallest reasonable change
* Preserve existing behavior unless intentionally changing the spec
* Do not refactor unrelated code
* Do not rename or restructure without clear justification
* Keep diffs focused and reviewable

---

## Dependency policy

* Prefer the Rust standard library where reasonable
* Add crates only when they clearly reduce complexity
* Avoid large or unnecessary dependencies
* Briefly justify new dependencies in the plan

---

## Filesystem rules

Always validate:

* Input exists
* Input is a file (file mode)
* Input is a directory (dir mode)
* Output directory is valid or creatable
* Output path derivation succeeds

Use `Path` / `PathBuf`.

Do not manually manipulate file paths as strings.

Do not assume UTF-8 paths unless required.

---

## ffmpeg execution

* Use `std::process::Command`
* Do NOT use shell wrappers
* Detect and report missing `ffmpeg`
* Surface meaningful error output on failure
* Keep invocation explicit and reproducible

---

## Error handling

* Use `anyhow` for application-level errors
* Provide clear, actionable error messages
* Do not panic for expected runtime errors
* Fail early on invalid input

Batch mode:

* Continue processing on individual failures (default)
* Support `--fail-fast` for early termination

---

## Result model

Each job produces:

```rust
enum JobResult {
    Converted,
    Skipped,
    Failed(String),
}
```

Batch execution must aggregate results into a summary.

---

## Testing expectations

### Unit tests

* Path derivation
* Output directory resolution
* Skip/overwrite logic

### Filesystem tests (use `tempfile`)

* Directory creation
* Invalid target path handling
* Existing output behavior

### Batch behavior

* Correct job discovery
* Parallel execution correctness

Avoid requiring real `ffmpeg` for most tests.

---

## Documentation requirements

Update `README.md` when changing:

* CLI syntax
* Flags
* Default behavior
* Parallelism model
* Output directory logic

CLI help text must always match actual behavior.

---

## Commenting philosophy

* Comment the *why*, not the obvious *what*
* Avoid redundant comments
* Document non-obvious constraints and decisions
* Keep public interfaces understandable

---

## CLI UX consistency

* Keep output concise and predictable
* Preserve stable CLI behavior
* Do not change flags or defaults casually
* Update help text and README when UX changes

---

## Determinism

Prefer deterministic behavior where possible:

* Stable file discovery order
* Predictable output paths
* Consistent summaries

Avoid hidden or surprising behavior.

---

## What to optimize for

When working in this repository, prioritize:

* Reliable conversions
* Predictable CLI behavior
* Safe filesystem operations
* Efficient bounded parallelism
* Clean, maintainable, testable code

