#!/usr/bin/env bats

setup() {
  export TEST_DIR="$(mktemp -d)"
  export INPUT_FILE="$TEST_DIR/test.flac"
  export TARGET_DIR="$TEST_DIR/output"

  # Override script TARGET_DIR via env hack (we'll patch it below)
  export FAKE_TARGET_DIR="$TARGET_DIR"

  # Create fake input file
  touch "$INPUT_FILE"

  # Create mock ffmpeg in PATH
  export PATH="$TEST_DIR/bin:$PATH"
  mkdir -p "$TEST_DIR/bin"

  cat > "$TEST_DIR/bin/ffmpeg" <<'EOF'
#!/usr/bin/env bash
# Fake ffmpeg: just create output file
output="${!#}"
touch "$output"
exit 0
EOF
  chmod +x "$TEST_DIR/bin/ffmpeg"
}

run_script() {
  run env TARGET_DIR="$TARGET_DIR" "$BATS_TEST_DIRNAME/../flac2aiff" "$@"
}

teardown() {
  rm -rf "$TEST_DIR"
}

@test "fails when no argument is provided" {
  run_script
  [ "$status" -ne 0 ]
  [[ "$output" == *"Usage:"* ]]
}

@test "fails when input file does not exist" {
  run_script "$TEST_DIR/missing.flac"
  [ "$status" -ne 0 ]
  [[ "$output" == *"not found"* ]]
}

@test "creates target directory if missing" {
  run_script "$INPUT_FILE"
  [ "$status" -eq 0 ]
  [ -d "$TARGET_DIR" ]
}

@test "fails if target path exists but is not a directory" {
  mkdir -p "$TEST_DIR"
  touch "$TARGET_DIR"  # file instead of dir

  run_script "$INPUT_FILE"
  [ "$status" -ne 0 ]
  [[ "$output" == *"NOT a directory"* ]]
}

@test "skips conversion if output already exists" {
  mkdir -p "$TARGET_DIR"
  touch "$TARGET_DIR/test.aiff"

  run_script "$INPUT_FILE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"Skipping"* ]]
}

@test "runs conversion and creates output file" {
  run_script "$INPUT_FILE"
  [ "$status" -eq 0 ]
  [ -f "$TARGET_DIR/test.aiff" ]
}

@test "prints success message on success" {
  run_script "$INPUT_FILE"
  [[ "$output" == *"Success"* ]]
}
