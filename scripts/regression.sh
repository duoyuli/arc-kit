#!/usr/bin/env bash
# Full regression gate: fmt check, check, clippy, test, CLI smoke with isolated ARC_KIT_USER_HOME.
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

echo "==> [5/5] CLI smoke (isolated home + empty cwd, no arc.toml)"
TMP_HOME="$(mktemp -d)"
TMP_CWD="$(mktemp -d)"
# shellcheck disable=SC2064
trap 'rm -rf "$TMP_HOME" "$TMP_CWD"' EXIT

# Run arc with cwd under TMP_CWD (no arc.toml) while building from workspace root.
run_arc() {
  (
    cd "$TMP_CWD"
    ARC_KIT_USER_HOME="$TMP_HOME" cargo run --manifest-path "$ROOT/Cargo.toml" -q -p arc-cli -- "$@"
  )
}

run_arc --help >/dev/null
run_arc version >/dev/null
run_arc status >/dev/null
run_arc status --format json >/dev/null
run_arc project --help >/dev/null
run_arc project apply --help >/dev/null
run_arc project edit --help >/dev/null
run_arc skill list >/dev/null
run_arc market list >/dev/null
run_arc provider list >/dev/null
run_arc completion zsh >/dev/null

echo "==> All regression checks passed."
