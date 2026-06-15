# arc-kit

> Provider, skill, market, and project configuration manager for coding agents.

[中文](README.zh-CN.md)

## What It Solves

When teams use multiple coding agents, the same setup work tends to be repeated in every tool:

- provider profiles are switched by editing each agent's native config;
- useful skills are copied into several agent directories by hand;
- skill updates require another manual copy pass;
- shared skill repositories must be cloned, scanned, and installed manually;
- project onboarding depends on local, undocumented state.

`arc-kit` puts those workflows behind one local CLI.

## Core Capabilities

### Provider Management

Switch provider profiles for supported agents:

```bash
arc provider list
arc provider use <name> --agent codex
arc provider test
```

Project-level provider requirements can be declared in `arc.toml` and applied with `arc project apply`.
When writing Codex proxy providers, `arc-kit` keeps Codex's native provider `name` field fixed as `OpenAI`; the arc provider profile name still selects the profile.

### Skill Management

Manage skills once under `~/.arc-cli/skills/`, then install them into supported agents:

```bash
arc skill list
arc skill info <name>
arc skill install <name> --agent claude --agent codex
```

Skill sources are resolved by priority:

| Source | Path | Purpose |
| --- | --- | --- |
| local | `~/.arc-cli/skills/<name>/` | user-defined skills |
| market | remote git repositories | team or community shared skills |
| built-in | embedded in the binary | skills shipped with arc-kit |

### Market Sync

Markets are git repositories that contain skills:

```bash
arc market list
arc market add <git-url>
arc market update
arc market remove <git-url-or-id>
```

`arc market update` rebuilds the catalog and refreshes only arc-tracked global skill installs.

### Project Configuration

Put `arc.toml` in a repository to declare project requirements:

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

Then run:

```bash
arc project apply
arc status
```

`arc.toml` supports only `version`, `provider`, `markets`, and `skills`. MCP and subagent management have been removed.

## Installation

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

Target platform: macOS.

## Command Overview

```text
arc                     # Show help
arc status              # Show project, agent, catalog, and action status
arc version             # Show version
arc completion <shell>  # Generate shell completions
arc provider list       # List providers
arc provider use        # Switch provider
arc provider test       # Test provider connectivity
arc market list         # List market sources
arc market add <url>    # Add a market source
arc market remove <git-url-or-id>
arc market update       # Update all market sources
arc skill list          # List skills
arc skill install       # Install a skill
arc skill uninstall     # Uninstall a skill
arc skill info          # Show skill details
arc project apply       # Apply arc.toml configuration
arc project edit        # Edit arc.toml skills interactively
```

Use `--format json` for automation where supported:

```bash
arc status --format json
arc project apply --format json --agent codex
```

## User Guide

### Quick Start

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit

arc --help
arc version
arc status
```

Add and install a skill:

```bash
arc market add https://github.com/example/skills.git
arc market update
arc skill install my-skill --agent claude --agent codex
```

Apply project requirements:

```bash
arc project apply
arc status
```

If the current repository has no `arc.toml`, interactive `arc project apply` opens the project skill editor so you can create one.

### Interaction Modes

Human-oriented commands use interactive UI only when stdin and stdout are TTYs and `--format json` is not present:

```bash
arc provider use
arc skill install
arc project apply
```

Automation should use explicit arguments and JSON output where supported:

```bash
arc status --format json
arc project apply --format json --agent codex
```

`--format json` takes precedence over TTY detection.

### Status

`arc status` reports:

- detected coding agents;
- whether the current repository has `arc.toml`;
- missing, partial, or unavailable project skills;
- provider alignment with project requirements;
- recommended next actions.

JSON output contains these top-level modules:

- `project`
- `agents`
- `catalog`
- `actions`

### Providers

Providers control how Claude Code and Codex connect to model APIs.

```bash
arc provider list
arc provider use
arc provider use official --agent codex
arc provider test
```

Rules:

- `arc provider` is equivalent to `arc provider list`.
- Non-interactive `provider use` requires a provider name.
- If the same provider name exists for multiple agents, pass `--agent`.
- Codex proxy providers are written to Codex with native `name = "OpenAI"`; the arc provider name still selects the profile.
- `provider test` exits with `1` if any tested provider fails.

Provider config files:

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

### Skills

List and inspect skills:

```bash
arc skill list
arc skill info my-skill
arc skill list --format json
```

Install and uninstall skills:

```bash
arc skill install my-skill --agent claude
arc skill install my-skill --agent claude --agent codex
arc skill uninstall my-skill --agent claude
arc skill uninstall my-skill --all
```

Global skill paths:

| Agent | Path |
| --- | --- |
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>` |

Project-level skill paths:

| Agent | Path |
| --- | --- |
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

OpenClaw uses directory copy for global skills and does not support project-level skills.

### Markets

Markets are git repositories that contain skills.

```bash
arc market list
arc market add https://github.com/team/skills.git
arc market update
arc market remove <git-url-or-id>
```

`arc market update` pulls markets, rebuilds the catalog, and refreshes arc-tracked global skill installs. It does not manage manually placed files in native agent directories.

Tracking metadata is stored at:

```text
~/.arc-cli/state/skills/installs.json
```

If tracking metadata is corrupted, arc quarantines it as `installs.corrupt.<unix_ts>.json` and continues from empty tracking state.

### Project Configuration

Project configuration lets a repository declare its provider, skill, and market requirements.

Common commands:

```bash
arc project apply
arc project apply --agent codex
arc project apply --all-agents
arc project edit
```

`arc project apply`:

- connects markets declared in `arc.toml`;
- switches to the required provider;
- installs project-level skills for selected agents.

Minimal `arc.toml`:

```toml
version = 1

[skills]
require = ["architecture-review"]
```

Fuller example:

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

Rules:

- `arc.toml` is the project configuration entry point.
- `arc project apply` is the operation that changes local state.
- `arc project edit` edits skill requirements interactively.
- `--agent` and `--all-agents` choose project-level skill install targets.
- `arc.toml` must not contain secrets.
- `[mcps]` and `[subagents]` have been removed and are rejected as unknown fields.

### Shell Completions

```bash
arc completion zsh
arc completion bash
arc completion fish
arc completion powershell
arc completion elvish
```

Generated files are written under:

```text
~/.arc-cli/completions/
```

Re-run completion generation after upgrading `arc-kit`.

### Recommended Workflows

Personal setup:

```bash
arc status
arc provider use
arc skill list
arc skill install <name>
```

Team onboarding:

```bash
arc project apply
arc status
```

Automation:

```bash
arc status --format json
arc project apply --format json --agent codex
```

## Interaction and Automation Design

This section defines command semantics for humans, scripts, and coding agents.

### Runtime Modes

`arc-kit` has two runtime modes:

| Mode | Condition |
| --- | --- |
| Interactive | stdin and stdout are TTYs, and `--format json` is not specified |
| Non-interactive | no TTY, or `--format json` is specified |

`--format json` takes precedence over TTY detection. A command run in a terminal with `--format json` must take the automation path and must not launch TUI or `dialoguer` flows.

### JSON and Exit Codes

JSON output uses a top-level `schema_version`. Current schema version: `"5"`.

`arc status --format json` contains:

- `project`
- `agents`
- `catalog`
- `actions`

Exit code conventions:

| Scenario | Exit Code |
| --- | --- |
| success | 0 |
| configuration parse failure | 1 |
| `status` reports missing, partial, or unavailable skills | 0 |
| non-interactive missing required parameters | 1 |
| `arc provider test` has failures | 1 |
| JSON serialization failure | 1 |

Write-command JSON can exit `0` with `ok == false` for expected non-mutating failures, such as a missing `arc.toml` in `arc project apply --format json`. Automation must inspect `ok` and `message`, not only the process exit code.

### JSON Coverage

Read commands must support `--format json` unless explicitly registered as exceptions.

Required JSON read commands:

- `arc status`
- `arc market list`
- `arc skill list`
- `arc skill info <name>`
- `arc provider list`
- `arc provider test`
- `arc project edit` structured failure result

Registered exceptions:

- `arc version`
- bare `arc` with no subcommand
- `arc completion`

JSON output must not contain ANSI escape sequences.

### Write Commands

If an interactive command provides a wizard, multi-select, confirmation, or editor, the non-interactive path must be explicit and must not read stdin.

Current one-shot paths:

| Command | Non-interactive path |
| --- | --- |
| `skill install` / `skill uninstall` | explicit name plus target agent or `--all` where applicable |
| `provider use` | explicit provider name, plus `--agent` when ambiguous |
| `market add` / `market remove` / `market update` | fully parameterized by command arguments |
| `project apply` | `--agent` or `--all-agents` when project skills need installation |
| `project edit` | interactive-only editor; JSON path returns a structured failure without opening an editor |

### Project Configuration Design

`arc.toml` supports:

- `version`
- `[provider]`
- `[[markets]]`
- `[skills]`

`[mcps]` and `[subagents]` have been removed and are rejected as unknown fields.

When `arc project apply` runs interactively without an `arc.toml`, it opens the project skill editor to create one. In non-interactive mode without `arc.toml`, plain text exits with `1`; JSON returns `WriteResult.ok == false` with exit code `0`.

### UI Boundaries

- Business logic belongs in `arc-core`.
- CLI command definitions and user output belong in `arc-cli`.
- TUI and `dialoguer` interactions belong only in `arc-tui`.
- `arc-core` must not print to stdout or depend on UI libraries.

List-style TUIs must clip each rendered line to the current terminal width. Do not rely on terminal auto-wrapping.

### Resource Family Baseline

The only complete resource family today is `skill`:

| Verb | Interactive behavior | Non-interactive behavior |
| --- | --- | --- |
| `list` | TTY browser with drill-down to details | pipeable text and stable JSON collection |
| `info` | detail view from list or direct lookup | explicit single-item lookup and stable JSON detail |
| `install` | omitting name launches a wizard | explicit name and target agent |
| `uninstall` | omitting name selects from installed items | explicit name and target agent or `--all` |

When adding another resource family, evaluate the full `list / info / install / uninstall` set for both human and agent support.

### Anti-Patterns

- judging only by TTY while ignoring `--format json`;
- mixing ANSI into JSON;
- calling `dialoguer::Input::interact()` outside interactive mode;
- placing filesystem or domain behavior in `arc-cli` when it belongs in `arc-core`;
- adding a read command without JSON output.

## Development Guide

### Environment

- Rust stable toolchain
- macOS target platform

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

### Required Checks

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

### Repository Structure

```text
.
├── arc-cli/          # CLI, clap command table, user output, JSON structs
├── arc-core/         # domain logic, install engine, provider, market, skill, detect, paths, io
├── arc-tui/          # interactive UI; only this crate depends on dialoguer
├── built-in/         # built-in skills and market index
├── scripts/
│   └── regression.sh # pre-release regression
└── Cargo.toml
```

### Module Ownership

- `arc-core`: business logic, state, filesystem operations, provider application, market sync, skill registry, install engine, detection, and project resolution.
- `arc-cli`: command definitions, command dispatch, user output, and JSON response shapes.
- `arc-tui`: interactive terminal UI, selectors, fuzzy browsing, wizard flows, and themes.

Do not put business logic in `arc-cli`. Do not put `dialoguer` interaction in `arc-core` or `arc-cli`.

### Documentation Requirements

Behavioral code changes must update the relevant README sections:

- product-facing capability changes;
- user workflows;
- CLI semantics, JSON, or interaction changes;
- build, test, release, or module-ownership changes;
- matching `README.zh-CN.md` Chinese mirror content.

Code comments and CLI prompts are English. Official documentation is maintained in `README.md` and `README.zh-CN.md`.

### Contribution Rules

- Keep each change focused.
- Include tests for behavior changes.
- Avoid unrelated refactors.
- Do not introduce unused dependencies.
- Use `arc-core::io` atomic write helpers for persistent writes.
- Keep terminal layout and interactive-mode checks near the CLI/TUI boundary.

### Release Rules

- Confirm the `main` push succeeds before pushing a release tag.
- Push tags separately.
- Do not run `git push origin main --tags`.

### Roadmap Notes

- P0: provider, market, and skill behavior must remain stable; changes need tests.
- P1: strengthen market/provider black-box and edge-case tests.
- P2: continue documenting configuration and provider schema behavior.
