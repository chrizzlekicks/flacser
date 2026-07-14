# AGENTS.md

## Repo overview

- `flacser` is a Rust CLI for converting FLAC, AIFF, and WAV files between supported lossless formats with `ffmpeg`.
- Main commands are `convert` for single-file or directory conversion and `doctor` for read-only environment checks.

## Repo structure

- `src/main.rs` wires CLI commands to the conversion and doctor flows.
- Core conversion logic lives in `src/` modules such as discover, plan, convert, ffmpeg, interrupt, progress, and summary.
- Integration tests live in `tests/`; see `ARCHITECTURE.md` for deeper design details.

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
4. Implement approved scope only; do not edit files before approval

---

## Change rules

- Keep changes minimal
- Do not refactor unrelated code
- Preserve existing behavior unless explicitly changing it

---

## Core goal

Reliable, predictable, and simple batch conversion with clean parallel execution.
