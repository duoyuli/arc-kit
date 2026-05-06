#!/usr/bin/env bash
# Full regression gate: fmt check, check, clippy, test, plus black-box CLI regression coverage.
# Run from repo root: ./scripts/regression.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> [1/5] cargo fmt --all --check"
cargo fmt --all --check

echo "==> [2/5] cargo check"
cargo check

echo "==> [3/5] cargo clippy (deny warnings)"
cargo clippy --all-targets -- -D warnings

echo "==> [4/5] cargo test"
cargo test

echo "==> [5/5] CLI regression (isolated home + black-box behavior checks)"
TMP_ROOT="$(mktemp -d)"
TMP_HOME="$TMP_ROOT/home"
TMP_CWD="$TMP_ROOT/empty-cwd"
TMP_BIN="$TMP_ROOT/bin"
TMP_STDOUT="$TMP_ROOT/stdout.txt"
TMP_STDERR="$TMP_ROOT/stderr.txt"
mkdir -p "$TMP_HOME" "$TMP_CWD" "$TMP_BIN"
# shellcheck disable=SC2064
trap 'rm -rf "$TMP_ROOT"' EXIT

create_empty_builtin_manifest() {
  local builtin_dir="$TMP_HOME/built-in"
  local market_dir="$builtin_dir/market"
  local manifest="$builtin_dir/manifest.toml"
  mkdir -p "$market_dir"
  printf 'version = 1\n\n[index.market]\npath = "market/index.toml"\n' >"$manifest"
  printf 'version = 1\nupdated_at = "2026-04-10"\n' >"$market_dir/index.toml"
  printf '%s' "$manifest"
}

BUILTIN_MANIFEST_PATH="$(create_empty_builtin_manifest)"
ARC_TEST_PATH="$TMP_BIN:$PATH"

write_fake_cli() {
  local name="$1"
  local version_flag="$2"
  local version_output="$3"
  local path="$TMP_BIN/$name"
  cat >"$path" <<EOF
#!/bin/sh
if [ "\$1" = "$version_flag" ]; then
  echo "$version_output"
fi
exit 0
EOF
  chmod +x "$path"
}

run_arc_in() {
  local cwd="$1"
  shift
  (
    cd "$cwd"
    ARC_KIT_USER_HOME="$TMP_HOME" \
    ARC_KIT_BUILTIN_MARKET_INDEX_URL="file://$BUILTIN_MANIFEST_PATH" \
    PATH="$ARC_TEST_PATH" \
    cargo run --manifest-path "$ROOT/Cargo.toml" -q -p arc-cli -- "$@"
  )
}

capture_arc_in() {
  local cwd="$1"
  shift
  run_arc_in "$cwd" "$@" >"$TMP_STDOUT" 2>"$TMP_STDERR"
}

require_exit_code() {
  local expected="$1"
  local context="$2"
  shift 2
  set +e
  capture_arc_in "$@"
  local status=$?
  set -e
  if [ "$status" -ne "$expected" ]; then
    echo "Expected exit $expected but got $status for: $context" >&2
    cat "$TMP_STDOUT" >&2 || true
    cat "$TMP_STDERR" >&2 || true
    exit 1
  fi
}

require_stdout() {
  local pattern="$1"
  local context="$2"
  if ! rg -q "$pattern" "$TMP_STDOUT"; then
    echo "Missing stdout pattern '$pattern' for: $context" >&2
    cat "$TMP_STDOUT" >&2 || true
    exit 1
  fi
}

require_stderr() {
  local pattern="$1"
  local context="$2"
  if ! rg -q "$pattern" "$TMP_STDERR"; then
    echo "Missing stderr pattern '$pattern' for: $context" >&2
    cat "$TMP_STDERR" >&2 || true
    exit 1
  fi
}

write_fake_cli "codex" "-V" "codex-cli 0.116.0"
write_fake_cli "claude" "-v" "2.1.84 (Claude Code)"
write_fake_cli "openclaw" "-v" "openclaw 1.0.0"
write_fake_cli "kimi" "--version" "kimi 1.0.0"

# Base smoke in empty cwd.
run_arc_in "$TMP_CWD" --help >/dev/null
run_arc_in "$TMP_CWD" version >/dev/null
run_arc_in "$TMP_CWD" status >/dev/null
require_exit_code 0 "status json in empty cwd" "$TMP_CWD" status --format json
require_stdout '"actions"' "status json exposes actions"
run_arc_in "$TMP_CWD" project --help >/dev/null
run_arc_in "$TMP_CWD" project apply --help >/dev/null
run_arc_in "$TMP_CWD" project edit --help >/dev/null
require_exit_code 0 "project edit json returns structured failure" "$TMP_CWD" project edit --format json
require_stdout '"ok": false' "project edit json exposes structured failure"
run_arc_in "$TMP_CWD" skill list >/dev/null
run_arc_in "$TMP_CWD" market list >/dev/null
run_arc_in "$TMP_CWD" provider list >/dev/null
run_arc_in "$TMP_CWD" completion zsh >/dev/null
require_exit_code 2 "mcp command is removed" "$TMP_CWD" mcp list
require_stderr "unrecognized subcommand 'mcp'" "mcp command is removed"
require_exit_code 2 "subagent command is removed" "$TMP_CWD" subagent list
require_stderr "unrecognized subcommand 'subagent'" "subagent command is removed"

# JSON non-interactive argument requirements.
require_exit_code 1 "skill install json requires name" "$TMP_CWD" skill install --format json
require_stderr "Skill name required" "skill install json requires name"
require_exit_code 1 "skill uninstall json requires name" "$TMP_CWD" skill uninstall --format json
require_stderr "Skill name required" "skill uninstall json requires name"
require_exit_code 1 "provider use json requires name" "$TMP_CWD" provider use --format json
require_stderr "Provider name required" "provider use json requires name"

# Structured JSON not-found paths.
require_exit_code 0 "skill info missing returns json error" "$TMP_CWD" skill info missing --format json
require_stdout '"ok": false' "skill info missing returns json error"
require_stdout '"error": "skill '\''missing'\'' not found\."' "skill info missing error message"

# Removed project resource sections are rejected by arc.toml parsing.
PROJ_REMOVED_RESOURCE="$TMP_ROOT/project-removed-resource"
mkdir -p "$PROJ_REMOVED_RESOURCE"
cat >"$PROJ_REMOVED_RESOURCE/arc.toml" <<'EOF'
[mcps]
require = ["github"]
EOF
require_exit_code 1 "project apply rejects removed mcp section" "$PROJ_REMOVED_RESOURCE" project apply
require_stderr 'unknown field "mcps"' "project apply rejects removed mcp section"

echo "==> All regression checks passed."
