## Context

`flacser` already follows a discover -> plan -> execute -> summarize pipeline. Planning determines the full job list before execution starts, and execution already collects structured `JobResult` values across a Rayon thread pool before the final summary is printed. The current gap is that users see no runtime feedback between the start of execution and the end-of-run summary, which is most noticeable for directory conversions and slower `ffmpeg` runs.

Progress tracking needs to fit the current job-isolation model: each conversion job remains independent, shared configuration stays read-only, and concurrent workers must not mutate shared state in an unsafe way. The design also needs to preserve simple CLI behavior for single-file runs and avoid introducing a heavyweight terminal UI dependency for an otherwise small tool. The desired output is intentionally minimal: one aggregate line per completed job in the form `[completed/total] done`.

## Goals / Non-Goals

**Goals:**
- Show users observable forward progress while conversions are running.
- Report progress in terms of completed jobs out of total planned jobs.
- Count converted, skipped, and failed outcomes in the progress signal.
- Preserve the existing final summary and failure reporting.
- Keep the implementation compatible with Rayon-based parallel execution.

**Non-Goals:**
- Building an interactive multi-line progress bar or rich TUI.
- Reporting byte-level or ffmpeg-internal encoding progress for each file.
- Changing planning, output mapping, or job scheduling behavior.

## Decisions

### Emit progress from the execution layer
The executor already knows the total job count and observes each job outcome, so progress reporting should live alongside `execute` rather than in `summary`. This keeps runtime feedback close to the parallel work and avoids reconstructing execution state after the fact.

Alternative considered: emit progress from `summary`.
This was rejected because `summary` only sees the fully completed `ExecutionReport`, so it cannot provide in-flight updates.

### Use a thread-safe progress reporter with serialized output
Parallel workers need a shared way to notify job completion without violating the existing independent job model. A small shared reporter based on atomic counters plus a synchronized print path keeps state minimal and avoids interleaved output from concurrent threads.

Alternative considered: let each worker print directly.
This was rejected because concurrent writes would produce noisy, hard-to-read output and make progress lines unreliable.

### Report completion milestones with a fixed aggregate format
`flacser` plans all jobs before execution, so it can reliably report `completed/total` progress even when individual jobs have different durations. The CLI should print a fixed line format, `[completed/total] done`, for each terminal job completion. When planning produces zero jobs, the CLI should print no progress lines and fall through to the existing summary output. This gives users predictable feedback without parsing `ffmpeg` output, exposing completion-order details, or coupling progress tracking to codec-specific behavior.

Alternative considered: include per-file paths or per-outcome labels in progress output.
This was rejected because aggregate-only progress is enough to show forward motion, keeps output calm during parallel runs, and leaves outcome detail to the final summary and failure reporting.

### Keep dry-run behavior consistent with execution progress
Dry-run mode already returns `Converted` results without spawning `ffmpeg`, so it should still advance progress for each planned job. This keeps the user-visible semantics aligned: dry-run simulates the same pipeline and still communicates how many jobs were processed.

## Risks / Trade-offs

- [Frequent progress output could be noisy for very large job counts] -> Keep the output to one short deterministic line, `[n/total] done`, per completed job.
- [Progress output can complicate test assertions] -> Keep formatting stable and cover it with focused tests around reporter behavior or captured CLI output.
- [Shared reporting state adds concurrency surface area] -> Limit shared state to counters and serialized printing, leaving conversion jobs and `ffmpeg` execution isolated.

## Migration Plan

No data or config migration is required. The change is limited to runtime CLI output and can ship as a backward-compatible enhancement.

## Open Questions

- Whether a future version should make progress output optional for scripting-oriented use cases.
