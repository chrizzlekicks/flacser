## Requirements

### Requirement: Convert command supports shorthand flag aliases
The `flacser convert` command SHALL accept documented single-letter shorthand aliases for supported options while preserving the existing long-form flags. The shorthand contract SHALL be `-o` for `--output-dir`, `-w` for `--overwrite`, `-n` for `--dry-run`, `-r` for `--recursive`, and `-j` for `--jobs`.

#### Scenario: Help output documents shorthand aliases
- **WHEN** a user runs `flacser convert --help`
- **THEN** the help output includes `-o, --output-dir <OUTPUT_DIR>`
- **THEN** the help output includes `-w, --overwrite`
- **THEN** the help output includes `-n, --dry-run`
- **THEN** the help output includes `-r, --recursive`
- **THEN** the help output includes `-j, --jobs <JOBS>`

#### Scenario: Shorthand boolean flags are accepted
- **WHEN** a user runs `flacser convert <directory> --dry-run -r`
- **THEN** the command behaves as if `--recursive` was provided and processes nested `.flac` files

#### Scenario: Shorthand value flags are accepted
- **WHEN** a user runs `flacser convert <input> -j 2`
- **THEN** the command accepts the invocation as if `--jobs 2` was provided

#### Scenario: Long-form flags remain valid
- **WHEN** a user runs `flacser convert <input> --dry-run --output-dir <dir>`
- **THEN** the command continues to behave exactly as it did before shorthand aliases were introduced
