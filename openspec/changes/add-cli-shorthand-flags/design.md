## Context

`flacser` exposes a single `convert` subcommand, and its current clap definition only declares long-form options in `src/cli.rs`. The existing integration suite in `tests/convert_cli.rs` already verifies help output and several option-driven behaviors, which makes it a good place to add contract tests before updating the clap attributes.

## Goals / Non-Goals

**Goals:**
- Add stable single-letter aliases for commonly used `convert` options.
- Preserve existing long-form flags and all current conversion behavior.
- Implement the change with tests that fail before the clap definition is updated.

**Non-Goals:**
- Renaming existing long-form flags or changing their semantics.
- Changing subcommand structure, positional arguments, or summary output.
- Introducing shorthand for the `convert` subcommand itself unless clap already provides it elsewhere.

## Decisions

1. Define short aliases directly on the existing clap field attributes in `ConvertArgs`.
Why: this is the smallest change, keeps the CLI contract in one place, and lets clap update parsing and generated help text together.
Alternative considered: manually parsing aliases before clap runs. Rejected because it adds avoidable custom logic and drifts from clap's help generation.

2. Use CLI integration tests as the primary TDD driver.
Why: the feature is user-facing behavior, so end-to-end argument parsing and help output checks cover the contract better than unit tests on struct metadata.
Alternative considered: parser-only unit tests. Rejected because they provide weaker coverage of the visible CLI experience.

3. Assign mnemonic short flags where there is an obvious one-letter mapping: `-o` for `--output-dir`, `-n` for `--dry-run`, `-r` for `--recursive`, and `-j` for `--jobs`.
Why: these letters are conventional and reduce surprise.
Alternative considered: arbitrary free letters. Rejected because they are harder to discover and remember.

4. Use `-w` for `--overwrite` to avoid colliding with `-o`, which is a stronger fit for output directory.
Why: `overwrite` has no perfect unique initial once `-o` is used, and `-w` is still mnemonic enough to document and test explicitly.
Alternative considered: giving `--overwrite` no shorthand. Rejected because overwrite is a common operational flag and benefits from parity with the other options.

## Risks / Trade-offs

- Help output changes may make exact-string tests brittle -> keep assertions focused on the expected option lines rather than full help snapshots.
- Short-flag selection is a user-facing contract -> document the chosen mappings in the spec and tests so later changes are intentional.
- Combined short-flag usage can introduce ambiguity around value-taking options -> rely on clap's standard parsing behavior and cover representative invocations in integration tests.
