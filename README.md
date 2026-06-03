# arc-kit

> 编码 Agent 的 provider、skill 与 market 配置管理工具
>
> Provider, skill, and market configuration manager for coding agents

## 简介 / Introduction

在同时使用 Claude Code、Codex 等多个 Agent 时，常见问题包括：

- 切换模型供应商时，每个工具都要单独修改配置文件
- 写了一个好用的 Skill，想让所有助手都能用，得手动复制到不同目录
- Skill 升级后又得手动复制一遍
- 接入 GitHub 上的 Skill 仓库时，需要手动 clone、定位目录并复制
- 团队协作时，每个人的配置都不一样

**arc-kit 用同一套 CLI 管理 provider、skill、market 和项目级 skill 落地。**

When using multiple agents like Claude Code and Codex at the same time, common pain points include:

- Switching providers requires editing config files in each tool separately
- A useful skill must be manually copied into each agent's directory
- Skill upgrades require repeating the same manual copy
- Integrating a skill repo from GitHub involves cloning, locating directories, and copying
- Team members end up with inconsistent configurations

**arc-kit unifies provider, skill, market, and project-level skill management in one CLI.**

## 核心能力 / Core Capabilities

**1. Provider 统一管理 / Unified Provider Management**

执行 `arc provider use <name> [--agent <agent>]` 可切换 provider profile。交互式模式按 coding agent 分 tab 展示，一次只看一个 agent 的 provider，支持方向键与 `h/j/k/l` 导航、`q` 退出。若需在项目内固定 provider，可在 `arc.toml` 中声明 `[provider]` 后执行 `arc project apply`。

Run `arc provider use <name> [--agent <agent>]` to switch provider profiles. In interactive mode, providers are shown in tabs grouped by agent, with arrow keys / `h/j/k/l` for navigation and `q` to quit. To pin a provider at the project level, declare `[provider]` in `arc.toml` and run `arc project apply`.

**2. Skill 一处管理，多处使用 / Manage Once, Use Everywhere**

本地 skill 目录为 `~/.arc-cli/skills/`。加入 catalog 后，可通过 `arc skill install <name>` 安装到目标 agent。

三层来源，高优先级覆盖低优先级：

| 来源 / Source | 路径 / Path | 说明 / Description |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 用户自定义 / User-defined |
| market | 远程 git 仓库 / Remote git repo | 社区或团队共享 / Shared by community or team |
| built-in | 嵌入 arc-kit 二进制 / Embedded in binary | 自带 Skill，首次使用自动释放 / Shipped with arc-kit, extracted on first use |

The local skill directory is `~/.arc-cli/skills/`. After adding to the catalog, install with `arc skill install <name>` to any target agent. Three sources with higher priority overriding lower, as shown in the table above.

**3. Market 发现与同步 / Market Discovery & Sync**

- 可接入官方或社区维护的 skill 仓库
- 可接入团队私有仓库：`arc market add <Git 仓库地址>`
- 拉取更新并刷新 catalog：`arc market update`

- Connect to official or community-maintained skill repos
- Connect to team-private repos: `arc market add <git-url>`
- Pull updates and refresh catalog: `arc market update`

**4. 项目级配置 / Project-Level Configuration**

在仓库中放置 `arc.toml` 后，执行 `arc project apply` 可同步 market、skill 和 provider 要求。该命令支持非交互式 `--format json` 输出，适用于 CI/CD；首次执行且仓库内还没有 `arc.toml` 时，交互式路径会先进入单屏 `Project Skills` 编辑器创建配置，非交互式纯文本会报错，JSON 会返回结构化失败结果。

> MCP 与 subagent 管理功能已移除；`arc.toml` 只接受 `provider`、`skills`、`markets` 和 `version`。

Place `arc.toml` in your repo, then run `arc project apply` to sync markets, skills, and provider requirements. The command supports `--format json` for CI/CD; on first run without an `arc.toml`, interactive mode opens a single-screen `Project Skills` editor, non-interactive plain text exits with error, and JSON returns a structured failure.

> MCP and subagent management have been removed; `arc.toml` only accepts `provider`, `skills`, `markets`, and `version`.

## FAQ

**Q: arc-kit 支持哪些 Agent？ / Which agents does arc-kit support?**

当前支持 Claude Code、Codex、Cursor CLI、OpenClaw、OpenCode、Gemini CLI、Kimi CLI。安装时自动检测已安装的 agent。

Currently supports Claude Code, Codex, Cursor CLI, OpenClaw, OpenCode, Gemini CLI, and Kimi CLI. Installed agents are auto-detected.

**Q: 安装 Skill 后，各个 Agent 的目录结构会是什么样？ / What does the directory layout look like after installing a skill?**

默认使用软链接安装（OpenClaw 除外，使用目录复制）：

| Agent | 全局 Skill 路径 / Global Skill Path |
|------|------|
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>`（目录复制 / directory copy） |

Symlinks are used by default (except OpenClaw, which uses directory copy), as shown above.

项目级 skill 由 `arc.toml` 定义，`arc project apply` 安装到仓库内的 agent 路径：

| Agent | 项目级 Skill 路径 / Project Skill Path |
|------|------|
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

> OpenClaw 不参与项目级安装。 / OpenClaw does not support project-level skills.

Project-level skills are defined in `arc.toml` and installed via `arc project apply` into in-repo agent paths, as shown above.

**Q: `arc market update` 会做什么？ / What does `arc market update` do?**

拉取所有 market 源的最新内容，重建索引。然后仅维护 **arc 已追踪** 的全局 skill 安装，不会删除手工放进 agent 目录的 skill。追踪元数据统一写入 `~/.arc-cli/state/skills/installs.json`；如果该文件损坏，arc 会自动将其隔离为 `installs.corrupt.<unix_ts>.json` 后按空状态继续。

Pulls the latest content from all market sources and rebuilds the index. It only maintains **arc-tracked** global skill installs and does not remove manually placed skills. Tracking metadata is written to `~/.arc-cli/state/skills/installs.json`; if this file is corrupted, arc automatically quarantines it as `installs.corrupt.<unix_ts>.json` and continues with an empty state.

## 安装与使用 / Installation & Usage

### Homebrew（推荐 / Recommended）

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

### 命令总览 / Command Overview

```text
arc                     # 显示帮助 / Show help
arc status              # 显示状态 / Show project/agents/catalog/actions status
arc version             # 显示版本 / Show version (no --format json)
arc completion <shell>  # 生成 shell 补全 / Generate shell completions
arc provider list       # 列出供应商 / List providers
arc provider use        # 切换供应商 / Switch provider
arc provider test       # 测试连通性 / Test provider connectivity
arc market list         # 列出 market 源 / List market sources
arc market add <url>    # 添加 market 源 / Add market source
arc market remove <git-url-or-id>  # 移除 market 源 / Remove market source
arc market update       # 更新 market 源 / Update all market sources
arc skill list          # 列出 skills / List skills
arc skill install       # 安装 skill / Install skill
arc skill uninstall     # 卸载 skill / Uninstall skill
arc skill info          # 显示 skill 详情 / Show skill details
arc project apply       # 应用项目配置 / Apply arc.toml configuration
arc project edit        # 交互式编辑 / Edit arc.toml skills interactively
```

`arc project edit` / 首次执行的 `arc project apply` 使用同一套单屏 skill 编辑器：可直接搜索 skill，`space` 勾选，`enter` 保存，`esc` 取消且不写文件。

`arc project edit` / first-run `arc project apply` share the same single-screen skill editor: search skills, `space` to toggle, `enter` to save, `esc` to cancel without writing.

## 文档 / Documentation

| 文档 / Document | 内容 / Content |
|------|------|
| [docs/user/guide.md](docs/user/guide.md) | 产品使用说明书 / User manual |
| [docs/developer/design.md](docs/developer/design.md) | 交互/非交互设计规范 / Interactive/non-interactive design spec |
| [docs/developer/development.md](docs/developer/development.md) | 开发贡献指南 / Development & contribution guide |
