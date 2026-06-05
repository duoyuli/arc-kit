# Contributing to arc-kit

[中文](CONTRIBUTING.zh-CN.md)

Thank you for contributing. This file is the short contribution entry point; detailed workflow and command semantics live in the developer docs.

## Before You Start

- Bugs: open an issue with reproduction steps, expected behavior, actual behavior, macOS version, and installed coding agents.
- Major features or refactors: discuss scope before implementation.
- Behavioral changes: include tests that cover the affected core path.

## Development Environment

- Rust stable toolchain
- macOS target platform

```bash
cargo check
cargo test
```

## Required Checks

Run at the repository root before submitting code:

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

## Documentation Sync

Code changes must update the relevant docs:

- root `README.md` for product-facing changes;
- `docs/user/guide.md` for user workflows;
- `docs/developer/design.md` for command semantics, JSON, or interaction design;
- `docs/developer/development.md` for development workflow or release gates;
- matching `.zh-CN.md` files for Chinese mirrors.

## Pull Request Guidelines

- Keep each PR focused.
- Avoid unrelated refactors.
- Describe what changed, why it changed, and which commands or disk layouts are affected.
- Document migration or compatibility impact for breaking changes.
- Do not introduce unused dependencies.

## More Detail

- [Development guide](docs/developer/development.md)
- [Interaction and JSON design](docs/developer/design.md)
- [User manual](docs/user/guide.md)
