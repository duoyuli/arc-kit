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
- **先看状态再写入**：排查时优先跑 `arc status`、`arc provider`、`arc skill list --installed`、`arc market list`，不要先假设问题在某一个子系统。
- **不要把裸 `arc` 当成 `status`**：`arc` 等同于 `arc --help`。
- 用户若在**非 TTY / CI** 下操作，提醒为写入类命令提供**显式参数**，并优先配合 `--format json`；对写入类 JSON，除了退出码，还要检查 `ok`。
- 若本 skill 与实际行为冲突，以本仓库代码和 `docs/user/guide.md` 为准；尤其关注 `project apply` / `project edit` 的 JSON 和退出码语义。

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

arc project apply       [--agent <id> ... | --all-agents]；无 arc.toml 时交互下可创建；有 arc.toml 时按配置安装 skill、切换 provider
arc project edit        交互式编辑 [skills] require
```

项目配置：在仓库中放置 `arc.toml`，从**当前工作目录向上**查找最近的配置文件。详见 `docs/user/guide.md`。

**自动化约定**：只读类应支持 `--format json`；带向导的写入类须提供显式参数，以便脚本/CI 在非 TTY 下一键执行。详见 `docs/developer/design.md`。

## Agent 常用调用

```bash
arc status --format json
arc provider --format json
arc provider test --format json
arc skill list --installed --format json
arc skill info <name> --format json
arc market list --format json
arc project apply --format json --agent codex
```

- 用户说“怎么没生效”时，先看 `arc status`，再按 provider / skill / market 分拆定位。
- 用户说“脚本里要稳定解析”时，优先 `--format json`；`arc version` 例外，仅文本。
- 用户说“帮我装到某个 agent”时，优先显式传 `--agent`，不要依赖自动目标推断。

### 退出码提示（脚本）

- `arc status`：只读；缺失 skill 时仍为 **0**，输出中可见提示。
- `arc provider test`：任一受测项失败则 **1**（含 JSON 模式）。
- `arc project apply`：错误场景并不都靠退出码表达；`--format json` 下可能 **exit 0 但 `ok: false`**。
- `arc project edit`：`--format json` 下返回 `ok: false`，但不执行编辑。

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

[openai]
display_name = "OpenAI Official"
api_key = "sk-xxx"
```

- 有 `base_url` 时写入 `~/.codex/config.toml` 的 `model_provider` + `model_providers.<name>`
- 无 `base_url` 时清除 `model_provider`（回到官方直连）
- `api_key` 写入 `~/.codex/auth.json`
- 当前切换逻辑只实际消费 `api_key` / `base_url`；不要把额外字段当成已生效配置。

### 切换与探测

```bash
arc provider                     # 同 list：列出 providers（只读），按 Agent 分组展示
arc provider use                 # 交互式选择要切换的 provider
arc provider use mirror          # 名称唯一时自动识别 agent
arc provider use mirror -a claude
arc provider test                # 测试各 agent **当前激活的** provider（无激活则提示后成功退出）
arc provider test mirror --agent claude
```

- 非交互模式下，`arc provider use` 必须给 `<name>`。
- 若同名 provider 同时存在于多个 agent，`arc provider use <name>` 会报歧义，需补 `--agent`。
- `arc provider test --agent <agent>` 在该 agent **没有 active provider** 时会报错；不带参数时则只测试“所有已激活”的 provider，没有激活项时成功退出。

## Skill

### 三个来源（优先级从高到低）

| 来源 | 路径 | 说明 |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 放入含 `SKILL.md` 的目录即可 |
| built-in | 嵌入在 arc 二进制中 | 首次使用自动释放到缓存 |
| market | 通过 `arc market add` 添加 | 远程 git 仓库中的 skill |

同名 skill 高优先级覆盖低优先级。

### 安装机制

Skill 默认通过**软链接**安装到各 coding agent 的 skills 目录；**OpenClaw**（`~/.openclaw/skills/`）为**目录复制**。**Cursor** 全局安装目录为 `~/.cursor/skills-cursor/<name>/`（项目内路径仍为 `.cursor/skills/<name>/`）。`arc market update` 重建 catalog 后会按 registry 维护 **arc 已追踪** 的全局安装（删除已消失的、仅在目标落后时刷新）。也可手动 `arc skill install <name> -a <agent>`。

```bash
arc skill list --installed
arc skill install                           # 交互式模糊搜索
arc skill install my-skill                  # 安装到所有已检测 agent
arc skill install my-skill -a claude
arc skill install my-skill -a claude -a codex
arc skill uninstall my-skill --all
arc skill uninstall my-skill -a claude
```

- 非交互模式下，`install` / `uninstall` 都必须给 skill 名。
- `arc skill install <name>` 未传 `--agent` 时，会安装到默认目标集合；给 agent 执行时，优先显式传 `--agent` 以避免装得过宽。
- `arc skill uninstall <name>` 未传 `--agent` 且未传 `--all` 时，会按“当前已安装目标”移除；若本来没装，会返回成功语义并提示 `not installed`。
- `arc skill info <name> --format json` 在 skill 不存在时，当前实现会直接报错，stdout 不保证有 JSON。

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

Market 源是包含 skill 的 git 仓库。裸 `arc market` 等同于 `arc market list`。

```bash
arc market add https://github.com/example/skills.git
arc market list
arc market remove example-skills   # 支持 git URL 或 source id
arc market update                  # 拉取并重新扫描；并维护全局 skill 安装
```

首次运行 `arc skill list` 时，若无本地 catalog，会自动从内置索引引导初始 market 源。

- `arc market add` 仅接受合法 git URL；当前允许 `https://`、`git://`、`ssh://`、`git@`、`file://`。
- `arc market remove` 只移除 market 源与 catalog 记录，**不会卸载**已装到各 agent 的 skill。
- `arc market update` 除了刷新索引，还会按当前 registry 自动清理失效的全局安装，并重装仍存在的全局 skill 到最新解析路径。

## Project Flow

`arc project apply` 会从当前目录向上查找最近的 `arc.toml`。对 agent 而言，最重要的是区分“交互创建配置”和“非交互应用配置”：

- 交互式且无 `arc.toml`：可以直接 `arc project apply`，会进入向导创建并应用。
- 非交互且无 `arc.toml`：纯文本路径报错；JSON 路径返回 `WriteResult` 且 `ok: false`。
- 已有 `arc.toml` 且存在缺失 skill：交互式可选目标 agent；非交互必须传 `--agent <id>`（可重复）或 `--all-agents`。
- `--agent` 与 `--all-agents` 互斥。
- `[[markets]]` 中声明但本地尚未配置的源，会在 `arc project apply` 时自动补到本地 market 配置。
- `arc project edit` 仅交互式；若用户在 CI、管道或 JSON 模式下要求编辑，应改为直接修改 `arc.toml` 文件，而不是调用该命令。

## 排障速查

```bash
arc status
arc provider
arc provider test
arc skill list --installed
arc market list
```

- “provider 切了但没生效”：先看 `arc provider` 当前 active，再检查 `~/.claude/settings.json` 或 `~/.codex/config.toml` / `~/.codex/auth.json`。
- “skill 找得到但项目里没加载”：先看 `arc status` 的 project 区块，再执行 `arc project apply --agent <id>`。
- “market 删了怎么 skill 还在”：这是正常行为；`arc market remove` 不会卸载已安装 skill。
- “脚本里返回 0 但结果不对”：优先检查 JSON 的 `ok` / `items`，尤其是 `project apply` 与 `project edit`。

## 关键路径

| 路径 | 用途 |
|------|------|
| `~/.arc-cli/` | arc 状态根目录 |
| `~/.arc-cli/arc.log` | 详细日志（配合 `--verbose`） |
| `~/.arc-cli/providers/` | provider 配置 |
| `~/.arc-cli/providers/active.toml` | 各 agent 当前激活的 provider |
| `~/.arc-cli/skills/` | 本地自定义 skill |
| `~/.arc-cli/markets/` | market 源与索引缓存 |
