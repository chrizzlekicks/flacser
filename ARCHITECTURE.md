# ARCHITECTURE.md

## Overview

flacser is a Rust CLI tool for converting `.flac` audio files to `.aiff` using `ffmpeg`.

It is designed as a batch-capable conversion engine with deterministic behavior, safe filesystem handling, and bounded parallel execution.

The CLI is a thin layer over a reusable core.

---

## Core principles

- Simplicity over complexity
- Readability over cleverness
- Batch-first design
- Deterministic behavior
- Thin wrapper over ffmpeg

---

## High-level flow

CLI → Config → Discover → Plan → Execute → Summarize → Output

`doctor` is a separate read-only path that probes the environment and, when provided, validates input/output/job-limit inputs without running conversions.

---

## Module structure

- cli.rs        → CLI parsing (clap)
- config.rs     → resolve flags, env, defaults
- discover.rs   → find input files
- plan.rs       → map inputs to outputs
- convert.rs    → execute jobs (Rayon)
- interrupt.rs  → interrupt flag handling and ctrlc handler
- ffmpeg.rs     → process spawning
- progress.rs   → track completed jobs and print progress
- summary.rs    → aggregate results
- doctor.rs     → readiness checks and environment diagnostics
- main.rs       → orchestration

---

## Data model

### ConversionJob

Represents one conversion unit.

struct ConversionJob {
    input: PathBuf,
    output: PathBuf,
}

---

### JobResult

Represents outcome of a job.

enum JobResult {
    Converted,
    Skipped,
    Failed(String),
}

---

### BatchSummary

Aggregated results:

- total
- converted
- skipped
- failed

---

## Pipeline

### 1. Discover

Input: file or directory

- file → single entry
- directory → scan for `.flac` files
- optional recursion

Output: Vec<PathBuf>

---

### 2. Plan

- derive output paths
- preserve relative structure
- validate output directory
- detect collisions

Output: Vec<ConversionJob>

---

### 3. Execute

- run jobs in parallel (Rayon)
- each job is independent
- report completed-job progress as work finishes
- collect JobResult
- keep integration coverage platform-agnostic where possible via fake `ffmpeg` helpers
- validate interrupt handling with dedicated coverage for the interrupt flag and signal handler

---

### 4. Summarize

- aggregate results
- print summary
- determine exit code

### Doctor

- probe `ffmpeg` availability and version
- check detected CPU cores and default worker calculation
- optionally validate an input path, output directory, and configured job limit
- return a read-only report with `ok`, `warn`, and `fail` checks
- exit non-zero when any required check fails

---

## Parallelism

- Use Rayon
- No thread-per-file

workers = min(job_count, configured_jobs)

Default:

configured_jobs = max(1, cpu_cores - 1)

---

## Job isolation

Each job must be independent:

- owns input/output paths
- no shared mutable state
- immutable config
- separate ffmpeg process

---

## Filesystem behavior

Directory mode:

- preserve relative structure

Example:

input/album/song.flac  
→ output/album/song.aiff

No flattening in v1.

---

## Error handling

- use anyhow
- fail early on invalid input
- no panics for runtime errors

Batch:

- continue on errors
- collect failures
- exit non-zero if any failed

---

## CLI layer

Responsibilities:

- parse arguments
- resolve config
- invoke pipeline
- print output

Must not contain core logic.

---

## Extensibility

Future additions:

- flatten mode
- fail-fast mode
- support conversion to `.wav`
- GUI (e.g. Tauri)

---

## Non-goals

- reimplementing ffmpeg
- async runtime (Tokio)
- WASM-first execution

---

## Summary

flacser is a simple pipeline:

independent jobs → parallel execution → aggregated results

Focus: reliability, predictability, simplicity
