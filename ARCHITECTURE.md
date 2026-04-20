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

---

## Module structure

- cli.rs        → CLI parsing (clap)
- config.rs     → resolve flags, env, defaults
- discover.rs   → find input files
- plan.rs       → map inputs to outputs
- convert.rs    → execute jobs (Rayon)
- ffmpeg.rs     → process spawning
- summary.rs    → aggregate results
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
- collect JobResult

---

### 4. Summarize

- aggregate results
- print summary
- determine exit code

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
- progress reporting
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
