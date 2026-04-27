## 1. Define the CLI contract with tests

- [x] 1.1 Update `tests/convert_cli.rs` help assertions to require the documented shorthand aliases for `output-dir`, `overwrite`, `dry-run`, `recursive`, and `jobs`.
- [x] 1.2 Add integration coverage showing shorthand flags are accepted for at least one boolean option and one value-taking option.

## 2. Implement shorthand parsing

- [x] 2.1 Update the clap field attributes in `src/cli.rs` to add the approved shorthand aliases without changing the existing long-form flags.
- [x] 2.2 Verify that the long-form options continue to parse and behave as before after shorthand support is added.

## 3. Validate the finished change

- [x] 3.1 Run the targeted CLI test suite and fix any parsing or help-output regressions.
- [x] 3.2 Run the full test suite to confirm the shorthand change does not break unrelated conversion behavior.
