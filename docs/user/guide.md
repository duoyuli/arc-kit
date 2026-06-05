# arc-kit User Guide

[中文](guide.zh-CN.md)

`arc-kit` is a local CLI for managing coding-agent providers, skills, markets, and project requirements.

Target platform: macOS.

## Quick Start

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

## Interaction Modes

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

## Status

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

## Providers

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
- `provider test` exits with `1` if any tested provider fails.

Provider config files:

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

## Skills

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

## Markets

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

## Project Configuration

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

## Shell Completions

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

## Recommended Workflows

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
