## ADDED Requirements

### Requirement: Conversion runs SHALL report aggregate job progress during execution
The system SHALL emit progress updates during conversion execution whenever a planned job reaches a terminal outcome, using the total planned job count for the run. Each progress update SHALL use the format `[completed/total] done`.

#### Scenario: Batch conversion reports progress as jobs complete
- **WHEN** a directory conversion run starts with more than one planned job
- **THEN** the CLI outputs progress updates in the form `[completed/total] done` before the final summary is printed

#### Scenario: Single-file conversion reports completion progress
- **WHEN** a single-file conversion run has one planned job
- **THEN** the CLI outputs `[1/1] done` before the final summary is printed

### Requirement: Progress updates SHALL account for all terminal job outcomes
The system SHALL advance progress for successful conversions, skipped jobs, and failed jobs so the reported completion count always matches the number of finished jobs.

#### Scenario: Existing output is skipped
- **WHEN** a planned job is skipped because its output already exists and overwrite is disabled
- **THEN** the progress output advances by one completed job using the same aggregate `[completed/total] done` format

#### Scenario: Conversion job fails
- **WHEN** a planned job fails during output directory creation or ffmpeg execution
- **THEN** the progress output advances by one completed job using the same aggregate `[completed/total] done` format and the run still prints the final summary with the failure recorded

### Requirement: Progress reporting SHALL preserve existing summary behavior
The system SHALL keep the end-of-run summary and non-zero exit behavior for failed jobs unchanged after progress updates are added.

#### Scenario: Run completes with at least one failure
- **WHEN** one or more jobs fail during a conversion run
- **THEN** the CLI still prints the final summary after progress updates and exits non-zero

#### Scenario: Dry run completes without invoking ffmpeg
- **WHEN** a dry-run conversion processes planned jobs
- **THEN** the CLI emits progress updates for the simulated completions and still prints the final summary

#### Scenario: No conversion jobs are planned
- **WHEN** discovery and planning produce zero conversion jobs for a run
- **THEN** the CLI emits no progress updates and still prints the final summary
