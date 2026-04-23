# AGENTS.md

## Project

flacser is a Rust CLI tool that converts `.flac` files to `.aiff` using `ffmpeg`.

It supports both single-file and directory-based batch conversion with parallel execution.

---

## CLI

Single command:

flacser convert <input-path>

- `<input-path>` can be a file or directory
- auto-detect mode (file vs directory)

Flags:

- `--output-dir`
- `--overwrite`
- `--dry-run`
- `--recursive`

---

## Defaults

Directory mode:

- non-recursive by default
- preserve relative directory structure
- skip existing outputs
- continue on errors
- exit non-zero if any job fails

---

## Parallelism

- Use **Rayon**
- No thread-per-file
- Workers:

min(job_count, configured_jobs)

Default jobs:

max(1, cpu_cores - 1)

---

## Architecture

Pipeline:

1. discover
2. plan
3. execute
4. summarize

Modules:

- cli
- discover
- plan
- convert
- ffmpeg
- summary

Keep CLI separate from core logic.

---

## Job isolation

Jobs must be independent:

- each job owns input/output paths
- no shared mutable state
- shared config is read-only
- detect output collisions before execution
- each job runs its own `ffmpeg` process

Return structured results instead of mutating shared state.

---

## Coding style

- Prefer simplicity over complexity
- Prefer readability over cleverness
- Avoid unnecessary abstractions and lifetimes
- Keep functions small and explicit

---

## Agent workflow

1. Inspect code
2. Propose plan
3. Ask for approval
4. Implement approved scope only

Do not edit files before a plan is approved.

---

## Change rules

- Keep changes minimal
- Do not refactor unrelated code
- Preserve existing behavior unless explicitly changing it

---

## Core goal

Reliable, predictable, and simple batch conversion with clean parallel execution.
