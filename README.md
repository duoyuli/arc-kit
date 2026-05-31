# arc-kit

> Unified provider, skill, and market management for coding agents.

[中文](README.zh-CN.md)

## Overview

When developers use multiple coding agents such as Claude Code, Codex, Cursor CLI, OpenCode, Gemini CLI, and Kimi CLI, they often run into the same operational problems:

- each agent stores provider configuration in a different place
- reusable skills must be copied into each agent manually
- skill upgrades require another round of manual copying
- GitHub-hosted skill repositories must be cloned, located, and installed by hand
- teams cannot easily share a consistent agent setup

**arc-kit provides one CLI for managing provider profiles, reusable skills, skill markets, and project-level skill configuration.**

## Core Features

**1. Unified provider management**

Run `arc provider use <name> [--agent <agent>]` to switch provider profiles. The interactive mode groups providers by coding agent, shows one agent at a time, supports arrow keys and `h/j/k/l` navigation, and exits with `q`. To pin a provider inside a project, declare `[provider]` in `arc.toml` and run `arc project apply`.

**2. Manage skills once, use them across agents**

Local skills live in `~/.arc-cli/skills/`. After a skill is added to the catalog, install it into a target agent with `arc skill install <name>`.

Skill sources are resolved in this order:

| Source | Path | Description |
|---|---|---|
| local | `~/.arc-cli/skills/<name>/` | User-defined skills |
| market | remote Git repositories | Community or team-shared skills |
| built-in | embedded in the arc-kit binary | Bundled skills extracted on first use |

**3. Market discovery and synchronization**

- Add official, community, or team skill repositories
- Add private team repositories with `arc market add <git-repository-url>`
- Pull updates and rebuild the catalog with `arc market update`

**4. Project-level configuration**

Place an `arc.toml` file in a repository and run `arc project apply` to sync required markets, skills, and provider settings. The command supports non-interactive `--format json` output for CI/CD. If no `arc.toml` exists yet, the interactive flow opens the single-screen `Project Skills` editor; non-interactive plain-text mode returns an error, while JSON mode returns a structured failure result.

> MCP and subagent management have been removed. `arc.toml` only accepts `provider`, `skills`, `markets`, and `version`.

## FAQ

**Q: Which agents does arc-kit support?**

arc-kit currently supports Claude Code, Codex, Cursor CLI, OpenClaw, OpenCode, Gemini CLI, and Kimi CLI. Installed agents are detected automatically.

**Q: Where does arc-kit install skills for each agent?**

Global skills are installed as symlinks by default. OpenClaw uses directory copies instead.

| Agent | Global skill path |
|---|---|
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>` |

Project-level skills are defined by `arc.toml` and installed by `arc project apply` into agent-specific paths inside the repository:

| Agent | Project skill path |
|---|---|
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

> OpenClaw does not participate in project-level skill installation.

**Q: What does `arc market update` do?**

It pulls all configured market sources and rebuilds the catalog. It then maintains only global skill installations tracked by arc-kit; manually copied skills in agent directories are not removed. Tracking metadata is stored in `~/.arc-cli/state/skills/installs.json`. If that file is corrupted, arc-kit moves it to `installs.corrupt.<unix_ts>.json` and continues with an empty state.

## Installation

### Homebrew

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

## Commands

```text
arc                     # Show help
arc status              # Show Project / Agents / Catalog / Actions status
arc version             # Show version
arc completion <shell>  # Generate shell completion
arc provider list       # List available model providers
arc provider use        # Switch provider profile
arc provider test       # Test provider connectivity
arc market list         # List market sources
arc market add <url>    # Add a market source
arc market remove <git-url-or-id>  # Remove a market source
arc market update       # Update all market sources
arc skill list          # List skills
arc skill install       # Install a skill
arc skill uninstall     # Uninstall a skill
arc skill info          # Show skill details
arc project apply       # Apply arc.toml configuration
arc project edit        # Edit arc.toml skills interactively
```

`arc project edit` and the first-run interactive `arc project apply` flow use the same single-screen skill editor: search directly, press `space` to select, `enter` to save, and `esc` to cancel without writing changes.

## Documentation

| Document | Description |
|---|---|
| [docs/user/guide.md](docs/user/guide.md) | User guide covering installation, status, providers, markets, skills, project configuration, and shell completion |
| [docs/developer/design.md](docs/developer/design.md) | Interactive and non-interactive design, JSON conventions, and implementation notes |
| [docs/developer/development.md](docs/developer/development.md) | Development and contribution guide |
