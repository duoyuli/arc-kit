# Interaction and Automation Design

[中文](design.zh-CN.md)

This document defines command semantics for humans, scripts, and coding agents. User-facing workflows are in the [user guide](../user/guide.md); implementation conventions are in [development.md](development.md).

## Runtime Modes

`arc-kit` has two runtime modes:

| Mode | Condition |
| --- | --- |
| Interactive | stdin and stdout are TTYs, and `--format json` is not specified |
| Non-interactive | no TTY, or `--format json` is specified |

`--format json` takes precedence over TTY detection. A command run in a terminal with `--format json` must take the automation path and must not launch TUI or `dialoguer` flows.

## JSON and Exit Codes

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

## JSON Coverage

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

## Write Commands

If an interactive command provides a wizard, multi-select, confirmation, or editor, the non-interactive path must be explicit and must not read stdin.

Current one-shot paths:

| Command | Non-interactive path |
| --- | --- |
| `skill install` / `skill uninstall` | explicit name plus target agent or `--all` where applicable |
| `provider use` | explicit provider name, plus `--agent` when ambiguous |
| `market add` / `market remove` / `market update` | fully parameterized by command arguments |
| `project apply` | `--agent` or `--all-agents` when project skills need installation |
| `project edit` | interactive-only editor; JSON path returns a structured failure without opening an editor |

## Project Configuration

`arc.toml` supports:

- `version`
- `[provider]`
- `[[markets]]`
- `[skills]`

`[mcps]` and `[subagents]` have been removed and are rejected as unknown fields.

When `arc project apply` runs interactively without an `arc.toml`, it opens the project skill editor to create one. In non-interactive mode without `arc.toml`, plain text exits with `1`; JSON returns `WriteResult.ok == false` with exit code `0`.

## UI Boundaries

- Business logic belongs in `arc-core`.
- CLI command definitions and user output belong in `arc-cli`.
- TUI and `dialoguer` interactions belong only in `arc-tui`.
- `arc-core` must not print to stdout or depend on UI libraries.

List-style TUIs must clip each rendered line to the current terminal width. Do not rely on terminal auto-wrapping.

## Resource Family Baseline

The only complete resource family today is `skill`:

| Verb | Interactive behavior | Non-interactive behavior |
| --- | --- | --- |
| `list` | TTY browser with drill-down to details | pipeable text and stable JSON collection |
| `info` | detail view from list or direct lookup | explicit single-item lookup and stable JSON detail |
| `install` | omitting name launches a wizard | explicit name and target agent |
| `uninstall` | omitting name selects from installed items | explicit name and target agent or `--all` |

When adding another resource family, evaluate the full `list / info / install / uninstall` set for both human and agent support.

## Anti-Patterns

- judging only by TTY while ignoring `--format json`;
- mixing ANSI into JSON;
- calling `dialoguer::Input::interact()` outside interactive mode;
- placing filesystem or domain behavior in `arc-cli` when it belongs in `arc-core`;
- adding a read command without JSON output.
