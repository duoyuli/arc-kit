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
    ARC_KIT_BUILTIN_MANIFEST_URL="file://$BUILTIN_MANIFEST_PATH" \
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
require_stdout '"mcps"' "status json exposes mcps"
require_stdout '"subagents"' "status json exposes subagents"
require_stdout '"actions"' "status json exposes actions"
run_arc_in "$TMP_CWD" project --help >/dev/null
run_arc_in "$TMP_CWD" project apply --help >/dev/null
run_arc_in "$TMP_CWD" project edit --help >/dev/null
run_arc_in "$TMP_CWD" skill list >/dev/null
run_arc_in "$TMP_CWD" mcp list >/dev/null
run_arc_in "$TMP_CWD" subagent list >/dev/null
run_arc_in "$TMP_CWD" market list >/dev/null
run_arc_in "$TMP_CWD" provider list >/dev/null
run_arc_in "$TMP_CWD" completion zsh >/dev/null

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
require_exit_code 0 "mcp info missing returns json error" "$TMP_CWD" mcp info missing --format json
require_stdout '"ok": false' "mcp info missing returns json error"
require_stdout '"error": "mcp '\''missing'\'' not found"' "mcp info missing error message"
require_exit_code 0 "subagent info missing returns json error" "$TMP_CWD" subagent info missing --format json
require_stdout '"ok": false' "subagent info missing returns json error"
require_stdout '"error": "subagent '\''missing'\'' not found"' "subagent info missing error message"

# Project capability retarget cleanup.
PROJ_RETARGET="$TMP_ROOT/project-retarget"
mkdir -p "$PROJ_RETARGET"
printf '# reviewer\n' >"$PROJ_RETARGET/reviewer.md"
cat >"$PROJ_RETARGET/arc.toml" <<'EOF'
[[mcps]]
name = "filesystem"
targets = ["claude", "codex"]
transport = "stdio"
command = "npx"

[[subagents]]
name = "reviewer"
targets = ["claude", "codex"]
prompt_file = "reviewer.md"
EOF
run_arc_in "$PROJ_RETARGET" project apply >/dev/null
cat >"$PROJ_RETARGET/arc.toml" <<'EOF'
[[mcps]]
name = "filesystem"
targets = ["claude"]
transport = "stdio"
command = "npx"

[[subagents]]
name = "reviewer"
targets = ["claude"]
prompt_file = "reviewer.md"
EOF
run_arc_in "$PROJ_RETARGET" project apply >/dev/null
if [ -e "$PROJ_RETARGET/.codex/agents/reviewer.toml" ]; then
  echo "Expected codex project subagent to be removed after target shrink" >&2
  exit 1
fi
if [ -f "$PROJ_RETARGET/.codex/config.toml" ] && rg -q 'filesystem' "$PROJ_RETARGET/.codex/config.toml"; then
  echo "Expected codex MCP config to be removed after target shrink" >&2
  cat "$PROJ_RETARGET/.codex/config.toml" >&2
  exit 1
fi
test -f "$PROJ_RETARGET/.claude/agents/reviewer.md"
rg -q 'filesystem' "$PROJ_RETARGET/.mcp.json"

# Project global fallback + status schema/runtime reflection.
PROJ_FALLBACK="$TMP_ROOT/project-fallback"
mkdir -p "$PROJ_FALLBACK"
cat >"$PROJ_FALLBACK/arc.toml" <<'EOF'
[[mcps]]
name = "github"
targets = ["kimi"]
transport = "streamable_http"
url = "https://api.github.com/mcp"
EOF
require_exit_code 0 "project apply global fallback json" "$PROJ_FALLBACK" project apply --format json --allow-global-fallback
require_stdout '"resource_kind": "mcp"' "project apply json reports mcp item"
require_stdout '"applied_scope": "global"' "project apply json reports global fallback"
require_exit_code 0 "status json reflects fallback install" "$PROJ_FALLBACK" status --format json
require_stdout '"mcps"' "status json has mcps module"
require_stdout '"subagents"' "status json has subagents module"
require_stdout '"actions"' "status json has actions module"
require_stdout '"name": "github"' "status json reports project mcp"
require_stdout '"applied_scope": "global"' "status json reflects existing fallback install"

echo "==> All regression checks passed."
