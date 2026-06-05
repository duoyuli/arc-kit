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

## Documentation

| Document | Purpose |
| --- | --- |
| [docs/user/guide.md](docs/user/guide.md) | complete user manual |
| [docs/developer/design.md](docs/developer/design.md) | interactive/non-interactive and JSON design rules |
| [docs/developer/development.md](docs/developer/development.md) | development workflow, gates, and repository structure |
| [CONTRIBUTING.md](CONTRIBUTING.md) | contribution entry point |

Chinese mirrors use the same path with `.zh-CN.md`.
