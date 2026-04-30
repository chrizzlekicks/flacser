## Why

Batch conversions can run for a while with no visible indication that work is advancing, especially when multiple files are being processed in parallel. Adding progress tracking makes long-running runs more predictable and helps users distinguish active work from stalls before only seeing the final summary.

## What Changes

- Add CLI progress reporting for conversion runs using a simple aggregate line in the form `[completed/total] done`.
- Advance the aggregate progress line for every terminal job outcome, including converted, skipped, failed, and dry-run jobs.
- Keep the existing final summary so progress feedback complements, rather than replaces, end-of-run reporting.
- Ensure progress tracking works for both single-file and directory batch runs, including parallel execution.

## Capabilities

### New Capabilities
- `conversion-progress-reporting`: Report observable conversion progress during execution of planned jobs.

### Modified Capabilities

## Impact

- Affected code: `src/convert.rs`, `src/summary.rs`, `src/main.rs`, and possibly CLI-facing tests.
- APIs/systems: terminal output behavior during conversion runs.
- Dependencies: likely no new external dependencies if progress updates can be implemented with the standard library and existing Rayon-based execution flow.
