# arc-kit 产品使用说明书 / arc-kit User Manual

## 1. 产品简介 / Product Overview

`arc-kit` 是一个给 coding agent 用的本地配置管理工具，当前管理三类内容：

`arc-kit` is a local configuration management tool for coding agents, currently managing three resource types:

- Provider：切换 Claude Code / Codex 当前使用的 provider
  Provider: switch the active provider for Claude Code / Codex
- Skill：给 agent 安装能力说明
  Skill: install capability descriptions for agents
- Market：接入和同步 skill 仓库
  Market: connect and sync skill repositories

项目配置通过 `arc.toml` 把 provider、skills 和 markets 写进仓库，然后用 `arc project apply` 落地。

Project configuration uses `arc.toml` to declare provider, skills, and markets in the repo, then applies them with `arc project apply`.

> MCP 与 subagent 管理功能已移除；相关命令和 `arc.toml` section 不再可用。
>
> MCP and subagent management have been removed; related commands and `arc.toml` sections are no longer available.

当前目标平台：`macOS`。 / Current target platform: `macOS`.

## 2. 五分钟上手 / Quick Start (5 Minutes)

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

验证环境 / Verify installation:

```bash
arc --help
arc version
arc status
```

安装 skill / Install a skill:

```bash
arc market add https://github.com/example/skills.git
arc market update
arc skill install my-skill --agent claude --agent codex
```

应用到项目 / Apply to project:

```bash
arc project apply
arc status
```

如果当前项目里没有 `arc.toml`，交互式执行 `arc project apply` 时会先帮你创建。

If there is no `arc.toml` in the current project, running `arc project apply` interactively will create one for you.

## 3. 交互式与自动化 / Interactive vs Automated

直接在终端里执行命令会进入面向人的交互界面，例如：

Running commands directly in the terminal enters the human-oriented interactive interface:

```bash
arc provider use
arc skill install
arc project apply
```

加上 `--format json` 会走自动化路径，不进入交互界面：

Adding `--format json` takes the automation path, bypassing the interactive interface:

```bash
arc status --format json
arc project apply --format json --agent codex
```

## 4. 状态检查 / Status Check

`arc status` 用来看当前环境是不是已经准备好。

`arc status` shows whether the current environment is ready.

它会告诉你 / It reports:

- 当前有没有识别到 agent / Whether agents are detected
- 当前项目有没有 `arc.toml` / Whether the project has `arc.toml`
- 项目要求的 skill 有没有落地 / Whether required project skills are installed
- provider 是否和项目要求一致 / Whether provider matches project requirements
- 下一步建议做什么 / Recommended next actions

常用命令 / Common commands:

```bash
arc status
arc status --format json
```

JSON 顶层模块包括 / Top-level JSON modules:

- `project`
- `agents`
- `catalog`
- `actions`

## 5. Provider 使用 / Using Providers

Provider 用来切换 Claude Code 和 Codex 当前使用的模型接入方式。

Providers control how Claude Code and Codex connect to model APIs.

```bash
arc provider list
arc provider use
arc provider use official --agent codex
arc provider test
```

规则摘要 / Rules summary:

- `arc provider` 等同于 `arc provider list` / `arc provider` is equivalent to `arc provider list`
- 非交互式下，`use` 必须显式写 provider 名 / In non-interactive mode, `use` requires an explicit provider name
- 如果同名 provider 出现在多个 agent，需要加 `--agent` / If the same provider name exists for multiple agents, add `--agent`
- `provider test` 只要有一项失败，退出码就是 `1` / `provider test` exits with `1` if any test fails

Provider 配置文件在 / Provider config files are at:

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

## 6. Skill 使用 / Using Skills

查看 skill / View skills:

```bash
arc skill list
arc skill info my-skill
arc skill list --format json
```

安装 skill / Install a skill:

```bash
arc skill install my-skill --agent claude
arc skill install my-skill --agent claude --agent codex
```

卸载 skill / Uninstall a skill:

```bash
arc skill uninstall my-skill --agent claude
arc skill uninstall my-skill --all
```

全局 skill 路径 / Global skill paths:

| Agent | 路径 / Path |
|------|------|
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>` |

项目级 skill 路径 / Project-level skill paths:

| Agent | 路径 / Path |
|------|------|
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

OpenClaw 使用目录复制，不支持项目级 skill。 / OpenClaw uses directory copy and does not support project-level skills.

## 7. Market 使用 / Using Markets

Market 是 skill 来源仓库。 / Markets are skill source repositories.

```bash
arc market list
arc market add https://github.com/team/skills.git
arc market update
arc market remove <git-url-or-id>
```

`arc market update` 会拉取所有 market，重建 catalog，并刷新 arc 已追踪的全局 skill 安装。追踪元数据统一写入 `~/.arc-cli/state/skills/installs.json`；如果该文件损坏，arc 会自动将其隔离为 `installs.corrupt.<unix_ts>.json` 后按空状态继续。

`arc market update` pulls all markets, rebuilds the catalog, and refreshes arc-tracked global skill installs. Tracking metadata is written to `~/.arc-cli/state/skills/installs.json`; if corrupted, arc automatically quarantines it as `installs.corrupt.<unix_ts>.json` and continues with an empty state.

## 8. 项目配置与 arc.toml / Project Configuration & arc.toml

项目功能用来把一组要求写进仓库，然后一键应用到当前项目。

Project features let you declare a set of requirements in the repo and apply them in one command.

常用命令 / Common commands:

```bash
arc project apply
arc project apply --agent codex
arc project apply --all-agents
arc project edit
```

`project apply` 会 / `project apply` will:

- 自动接入 `arc.toml` 中声明的 market / Auto-connect markets declared in `arc.toml`
- 自动切换项目要求的 provider / Auto-switch to the project-required provider
- 自动安装项目级 skill / Auto-install project-level skills

最简单的 `arc.toml` / Simplest `arc.toml`:

```toml
version = 1

[skills]
require = ["architecture-review"]
```

常用例子 / Common example:

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

规则摘要 / Rules summary:

- `arc.toml` 是项目配置入口 / `arc.toml` is the project config entry point
- `project apply` 是真正落地 / `project apply` is the actual application step
- `project edit` 是交互式修改 skill require / `project edit` interactively edits skill requirements
- `--agent` / `--all-agents` 影响项目级 skill 安装目标 / `--agent` / `--all-agents` control project-level skill install targets
- `arc.toml` 不保存 secret / `arc.toml` does not store secrets
- `[mcps]` 与 `[subagents]` 已移除，出现时会被当作未知字段拒绝 / `[mcps]` and `[subagents]` have been removed and are rejected as unknown fields

## 9. Shell 补全 / Shell Completions

```bash
arc completion zsh
arc completion bash
arc completion fish
arc completion powershell
arc completion elvish
```

生成文件会写到 / Generated files are written to:

```text
~/.arc-cli/completions/
```

升级 `arc-kit` 后，建议重新执行一次补全生成命令。

After upgrading `arc-kit`, re-run the completion generation command.

## 10. 推荐使用路径 / Recommended Workflows

个人使用 / Personal use:

```bash
arc status
arc provider use
arc skill list
arc skill install <name>
```

团队项目接入 / Team project onboarding:

```bash
arc project apply
arc status
```

自动化 / Agent 场景 / Automation / Agent scenarios:

```bash
arc status --format json
arc project apply --format json --agent codex
```
