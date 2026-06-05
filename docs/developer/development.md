# Development Guide

[中文](development.zh-CN.md)

This document covers contributor workflow, repository structure, and verification gates. Command semantics are defined in [design.md](design.md); user workflows are in [../user/guide.md](../user/guide.md).

## Environment

- Rust stable toolchain
- macOS target platform

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

## Required Checks

Before submitting code:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

If CLI entry points, output formats, or interaction semantics changed, also run:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

Before version bumps, `v*` tags, or formal releases:

```bash
./scripts/regression.sh
```

The regression script runs formatting, build, clippy, tests, and black-box CLI checks in an isolated `ARC_KIT_USER_HOME`.

## Repository Structure

```text
.
├── arc-cli/          # CLI, clap command table, user output, JSON structs
├── arc-core/         # domain logic, install engine, provider, market, skill, detect, paths, io
├── arc-tui/          # interactive UI; only this crate depends on dialoguer
├── built-in/         # built-in skills and market index
├── docs/             # official documentation
├── scripts/
│   └── regression.sh # pre-release regression
└── Cargo.toml
```

## Module Ownership

- `arc-core`: business logic, state, filesystem operations, provider application, market sync, skill registry, install engine, detection, and project resolution.
- `arc-cli`: command definitions, command dispatch, user output, and JSON response shapes.
- `arc-tui`: interactive terminal UI, selectors, fuzzy browsing, wizard flows, and themes.

Do not put business logic in `arc-cli`. Do not put `dialoguer` interaction in `arc-core` or `arc-cli`.

## Documentation Requirements

Behavioral code changes must update the relevant documentation:

- `README.md` for product-facing capability changes;
- `docs/user/guide.md` for user workflows;
- `docs/developer/design.md` for CLI semantics, JSON, or interaction changes;
- `docs/developer/development.md` for build, test, release, or module-ownership changes;
- matching `.zh-CN.md` mirrors.

Code comments and CLI prompts are English. Official documentation is maintained as English-default files plus Chinese mirror files.

## Contribution Rules

- Keep each change focused.
- Include tests for behavior changes.
- Avoid unrelated refactors.
- Do not introduce unused dependencies.
- Use `arc-core::io` atomic write helpers for persistent writes.
- Keep terminal layout and interactive-mode checks near the CLI/TUI boundary.

## Release Rules

- Confirm the `main` push succeeds before pushing a release tag.
- Push tags separately.
- Do not run `git push origin main --tags`.

## Roadmap Notes

- P0: provider, market, and skill behavior must remain stable; changes need tests.
- P1: strengthen market/provider black-box and edge-case tests.
- P2: continue documenting configuration and provider schema behavior.
