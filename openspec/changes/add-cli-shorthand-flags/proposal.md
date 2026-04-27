## Why

The `flacser convert` command currently exposes only long-form options, which makes common invocations more verbose than necessary even though clap supports short aliases out of the box. Adding short flags now improves CLI ergonomics while the command surface is still small, and the user explicitly wants the change introduced with tests driving the behavior.

## What Changes

- Add short-form aliases for supported `flacser convert` options so users can invoke common flags with a single-letter form.
- Preserve all existing long-form options and command behavior; shorthand support is additive.
- Define and verify the CLI contract with tests before implementation so help output and parsing behavior stay stable.

## Capabilities

### New Capabilities
- `cli-shorthand-flags`: Defines the short-form flag contract for `flacser convert`, including parsing and help output expectations.

### Modified Capabilities

## Impact

- Affected code: `src/cli.rs`, CLI-facing integration tests in `tests/convert_cli.rs`
- User-facing API: command-line interface for `flacser convert`
- Dependencies: existing `clap` support only; no new crates expected
