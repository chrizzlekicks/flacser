## 1. Progress Reporting Core

- [x] 1.1 Add execution-layer progress reporting that tracks completed jobs against the total planned jobs during Rayon-based conversion runs.
- [x] 1.2 Ensure progress advancement covers converted, skipped, failed, and dry-run job outcomes without changing final `ExecutionReport` contents.

## 2. CLI Output Integration

- [x] 2.1 Implement the fixed CLI progress output format `[completed/total] done` so it remains readable during parallel execution and preserves the existing final summary.
- [x] 2.2 Keep failure details and exit behavior unchanged after progress updates are introduced.

## 3. Verification

- [x] 3.1 Add or update unit/integration tests that verify `[completed/total] done` output appears before the summary for single-job and multi-job runs.
- [x] 3.2 Add coverage for skipped, failed, dry-run, and zero-job scenarios to confirm progress counts stay aligned with terminal job outcomes and remain silent when no jobs are planned.
