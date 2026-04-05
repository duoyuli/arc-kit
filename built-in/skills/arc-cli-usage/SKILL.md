---
name: arc-cli-usage
description: >
  arc-kit（`arc` CLI）的安装、日常命令、provider/skill/market、`arc.toml` 与 `arc project apply|edit` 的一站式说明。
  只要用户提到 arc-kit、brew 安装 arc、切换 API/镜像/代理、Claude Code 与 Codex 的 provider、
  skill 的安装卸载与来源优先级、market 源与 catalog、项目级 skill 与全局 skill、
  `arc.toml` / `[skills] require`、OpenClaw 复制安装、Cursor skills 路径、脚本里用 `--format json`、
  或任何「这个 arc 命令怎么用」「装好了但找不到命令」类问题，都应优先使用本 skill。
  英文场景同样适用：arc CLI, Homebrew arc-kit, switch LLM provider, install agent skills, market catalog,
  MCP-adjacent tooling for coding agents on macOS.
  即使用户只说「帮我配一下代理」「skill 没装上」「market 怎么加」，只要上下文里可能出现 arc-kit，也应触发。
---

# arc-kit 使用指南

## 对 Agent 的指引

- **权威文档**：行为与边界情况以仓库内 `docs/user/guide.md` 为准；本 skill 是速查。交互/自动化语义另见 `docs/developer/design.md`。
- 用户若在**非 TTY / CI** 下操作，提醒为写入类命令提供**显式参数**，并可用 `--format json` 做稳定解析（例外见下文）。

## 安装

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

升级：`brew upgrade arc-kit`。

从源码构建：

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo install --path arc-cli --force
```

若找不到 `arc`，将 Cargo bin 加入 PATH，例如：`export PATH="$HOME/.cargo/bin:$PATH"`。

## 全局选项

| 选项 | 说明 |
|------|------|
| `-v, --verbose` | 未设置 `ARC_LOG` 时，日志级别默认按 `debug`（无 `-v` 且未设置 `ARC_LOG` 时内部默认 `info`）。日志写入 `~/.arc-cli/arc.log` 仅在经过状态初始化且 `init_logger` 的命令上；裸 `arc`、`arc version`、`arc completion` 等快路径不写该文件 |
| `--format` | `text`（默认）或 `json`：JSON 带稳定 schema，不混入 ANSI；**`--format json` 优先于 TTY** |

只读类命令一般支持 `--format json`；**`arc version`** 仅文本，不支持 JSON。`arc project edit` 在 JSON 模式下不执行编辑（返回错误语义）。完整列表见 `docs/user/guide.md`。

## 核心命令

```text
arc                     等同于 arc --help（不是 status）
arc status              market / agent / skill 汇总（含 arc.toml 项目上下文）
arc version             版本号（仅文本，无 --format json）
arc completion <shell>  生成 shell 补全（bash / zsh / fish / powershell / elvish）

arc provider            等同于 arc provider list
arc provider use        交互式选择；或 arc provider use <name> [-a|--agent <agent>]
arc provider test       探测连通性；可 arc provider test <name> [--agent …]

arc market list         列出 market 源；裸 arc market 同 list
arc market add <url>    添加 git market 源
arc market remove …     git URL 或 source id（内置源不可删）
arc market update       拉取并重建索引，并维护全局 skill 安装

arc skill list          列出 skill；--installed 仅已安装
arc skill install       交互式；或 arc skill install <name> [-a|--agent …]
arc skill uninstall …   [-a|--agent …] 或 --all
arc skill info <name>   详情（skill 不存在时 --format json 可能无 JSON 直接报错，见用户手册）

arc project apply       无 arc.toml：交互下向导创建并应用；**非交互且无 arc.toml** 会失败（见用户手册）。有 arc.toml：按配置安装 skill、切换 provider
arc project edit        交互式编辑 [skills] require
```

项目配置：在仓库中放置 `arc.toml`，从**当前工作目录向上**查找最近的配置文件。详见 `docs/user/guide.md`。

**自动化约定**：只读类应支持 `--format json`；带向导的写入类须提供显式参数，以便脚本/CI 在非 TTY 下一键执行。详见 `docs/developer/design.md`。

### 退出码提示（脚本）

- `arc status`：非交互下若 `arc.toml` 中 required skill 缺失且可装，可能以 **1** 退出。
- `arc provider test`：任一受测项失败则 **1**（含 JSON 模式）。

## Provider

### 配置文件

Provider 定义在 `~/.arc-cli/providers/<agent>.toml`，每个 section 是一个 provider profile。当前支持的 agent：`claude`、`codex`。

**Claude Code**（`claude.toml`）示例：

```toml
[mirror]
display_name = "Mirror"
description = "Mirror API proxy"
ANTHROPIC_BASE_URL = "https://mirror.example.com"
ANTHROPIC_AUTH_TOKEN = "sk-xxx"

[official]
display_name = "Official"
description = "Anthropic direct"
```

可选字段：`ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_DEFAULT_HAIKU_MODEL`、`ANTHROPIC_DEFAULT_SONNET_MODEL`、`ANTHROPIC_DEFAULT_OPUS_MODEL`。

切换时写入 `~/.claude/settings.json` 的 `env` 字段。

**Codex**（`codex.toml`）示例：

```toml
[mirror]
display_name = "Mirror"
description = "Third-party proxy"
api_key = "sk-xxx"
base_url = "https://proxy.example.com"
wire_api = "responses"

[openai]
display_name = "OpenAI Official"
api_key = "sk-xxx"
```

- 有 `base_url` 时写入 `~/.codex/config.toml` 的 `model_provider` + `model_providers.<name>`
- 无 `base_url` 时清除 `model_provider`（回到官方直连）
- `api_key` 写入 `~/.codex/auth.json`

### 切换与探测

```bash
arc provider                     # 同 list：列出 providers（只读），按 Agent 分组展示
arc provider use               # 交互式选择要切换的 provider
arc provider use mirror          # 名称唯一时自动识别 agent
arc provider use mirror -a claude
arc provider test                # 测试各 agent **当前激活的** provider（无激活则提示后成功退出）
arc provider test mirror --agent claude
```

## Skill

### 三个来源（优先级从高到低）

| 来源 | 路径 | 说明 |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 放入含 `SKILL.md` 的目录即可 |
| built-in | 嵌入在 arc 二进制中 | 首次使用自动释放到缓存 |
| market | 通过 `arc market add` 添加 | 远程 git 仓库中的 skill |

同名 skill 高优先级覆盖低优先级。

### 安装机制

Skill 默认通过**软链接**安装到各 coding agent 的 skills 目录；**OpenClaw**（`~/.openclaw/skills/`）为**目录复制**。**Cursor** 全局安装目录为 `~/.cursor/skills-cursor/<name>/`（项目内路径仍为 `.cursor/skills/<name>/`）。`arc market update` 重建 catalog 后会按 registry 维护全局安装（删除已消失的、重装仍存在的）。也可手动 `arc skill install <name> -a <agent>`。

```bash
arc skill list --installed
arc skill install                           # 交互式模糊搜索
arc skill install my-skill                  # 安装到所有已检测 agent
arc skill install my-skill -a claude
arc skill install my-skill -a claude -a codex
arc skill uninstall my-skill --all
arc skill uninstall my-skill -a claude
```

### 项目级 vs 全局（`arc project apply`）

`arc skill install` 为**全局**（用户家目录下各 agent）。`arc.toml` 中 `[skills] require` 由 `arc project apply` 落到**仓库内**路径；非交互下须 `--agent`（可重复）或 `--all-agents`。**OpenClaw 不参与**项目级安装，需用 `arc skill install` 装到 `~/.openclaw/skills`。

| Agent | 仓库内项目级 skill 路径 |
|-------|-------------------------|
| Claude | `.claude/skills/<name>/` |
| Codex | `.agents/skills/<name>/` |
| Cursor | `.cursor/skills/<name>/` |
| OpenCode | `.opencode/skills/<name>/` |
| Gemini | `.gemini/skills/<name>/` |
| Kimi | `.kimi/skills/<name>/` |

## Market

Market 源是包含 skill 的 git 仓库。

```bash
arc market add https://github.com/example/skills.git
arc market list
arc market remove example-skills   # 支持 git URL 或 source id
arc market update                  # 拉取并重新扫描；并维护全局 skill 安装
```

首次运行 `arc skill list` 时，若无本地 catalog，会自动从内置索引引导初始 market 源。

## 关键路径

| 路径 | 用途 |
|------|------|
| `~/.arc-cli/` | arc 状态根目录 |
| `~/.arc-cli/arc.log` | 详细日志（配合 `--verbose`） |
| `~/.arc-cli/providers/` | provider 配置 |
| `~/.arc-cli/providers/active.toml` | 各 agent 当前激活的 provider |
| `~/.arc-cli/skills/` | 本地自定义 skill |
| `~/.arc-cli/markets/` | market 源与索引缓存 |
