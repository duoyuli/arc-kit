# AGENTS.md

Minimal working constraints for agents in this repository.

## Project

- `arc-kit` is a Rust CLI for managing coding agent providers, skills, markets, and project configuration.
- Target platform: macOS only.
- The Cargo workspace is split into three crates:
  - `arc-cli` — CLI, command definitions, user-facing output.
  - `arc-core` — domain logic, state, and filesystem operations.
  - `arc-tui` — interactive terminal UI.

## Mandatory Rules

- Put business logic in `arc-core`, never in `arc-cli`.
- Put TUI / `dialoguer` interactions only in `arc-tui`.
- Every behavioral change must include tests.
- Every code change must update both `README.md` and `README.zh-CN.md`.
- No unrelated refactors. No unused dependencies.
- Code comments and CLI prompts are English-only. Documentation is bilingual (English + Chinese), English first.

## CLI Semantics

- The CLI has exactly two modes: **interactive** and **non-interactive**.
- Read commands must support `--format json` (unless registered as an exception).
- Write commands with wizards must provide a non-interactive parameter path. Non-interactive mode must never read stdin.
- JSON output must never contain ANSI escape codes. Exit-code semantics must be documented.
- Resource commands (`skill`, `mcp`, `subagent`) are evaluated as a full `list / info / install / uninstall` family, for both human and agent use.

## Verification

Run before every commit:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

If CLI commands changed, also run:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

Run before every release:

```bash
./scripts/regression.sh
```

## Release

- Confirm the `main` push succeeds first, then push the tag separately.
- Never run `git push origin main --tags`.

## Style

- Keep implementation and interaction simple, direct, and reliable.
