# arc-kit

> 面向 coding agent 的 provider、skill、market 与项目配置管理工具。

[English](README.md)

## 解决什么问题

团队同时使用多个 coding agent 时，同一类配置工作往往要在每个工具里重复执行：

- provider profile 需要分别编辑每个 agent 的原生配置；
- 好用的 skill 需要手工复制到多个 agent 目录；
- skill 更新后还要重新复制；
- 共享 skill 仓库需要手工 clone、扫描和安装；
- 项目接入依赖本机未记录的状态。

`arc-kit` 用一个本地 CLI 收敛这些流程。

## 核心能力

### Provider 管理

为支持的 agent 切换 provider profile：

```bash
arc provider list
arc provider use <name> --agent codex
arc provider test
```

项目级 provider 要求可以写在 `arc.toml` 中，并通过 `arc project apply` 落地。

### Skill 管理

在 `~/.arc-cli/skills/` 中统一管理 skill，然后安装到支持的 agent：

```bash
arc skill list
arc skill info <name>
arc skill install <name> --agent claude --agent codex
```

Skill 来源按优先级解析：

| 来源 | 路径 | 用途 |
| --- | --- | --- |
| local | `~/.arc-cli/skills/<name>/` | 用户自定义 skill |
| market | 远程 git 仓库 | 团队或社区共享 skill |
| built-in | 嵌入二进制 | arc-kit 自带 skill |

### Market 同步

Market 是包含 skill 的 git 仓库：

```bash
arc market list
arc market add <git-url>
arc market update
arc market remove <git-url-or-id>
```

`arc market update` 会重建 catalog，并且只刷新由 arc 跟踪的全局 skill 安装。

### 项目配置

在仓库中放置 `arc.toml` 来声明项目要求：

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

然后执行：

```bash
arc project apply
arc status
```

`arc.toml` 只支持 `version`、`provider`、`markets` 和 `skills`。MCP 与 subagent 管理功能已移除。

## 安装

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

目标平台：macOS。

## 命令总览

```text
arc                     # 显示帮助
arc status              # 显示项目、agent、catalog 和 action 状态
arc version             # 显示版本
arc completion <shell>  # 生成 shell 补全
arc provider list       # 列出 provider
arc provider use        # 切换 provider
arc provider test       # 测试 provider 连通性
arc market list         # 列出 market 源
arc market add <url>    # 添加 market 源
arc market remove <git-url-or-id>
arc market update       # 更新所有 market 源
arc skill list          # 列出 skill
arc skill install       # 安装 skill
arc skill uninstall     # 卸载 skill
arc skill info          # 显示 skill 详情
arc project apply       # 应用 arc.toml 配置
arc project edit        # 交互式编辑 arc.toml skills
```

支持时，可用 `--format json` 进行自动化：

```bash
arc status --format json
arc project apply --format json --agent codex
```

## 用户指南

### 快速开始

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit

arc --help
arc version
arc status
```

添加并安装 skill：

```bash
arc market add https://github.com/example/skills.git
arc market update
arc skill install my-skill --agent claude --agent codex
```

应用项目要求：

```bash
arc project apply
arc status
```

如果当前仓库没有 `arc.toml`，交互式 `arc project apply` 会打开项目 skill 编辑器，帮助创建配置。

### 交互模式

只有当 stdin 和 stdout 都是 TTY，并且没有使用 `--format json` 时，面向人的命令才会进入交互 UI：

```bash
arc provider use
arc skill install
arc project apply
```

自动化场景应使用显式参数，并在支持时使用 JSON 输出：

```bash
arc status --format json
arc project apply --format json --agent codex
```

`--format json` 优先于 TTY 检测。

### 状态检查

`arc status` 会报告：

- 已检测到的 coding agent；
- 当前仓库是否存在 `arc.toml`；
- 缺失、部分落地或不可用的项目 skill；
- provider 是否符合项目要求；
- 推荐的下一步操作。

JSON 输出包含这些顶层模块：

- `project`
- `agents`
- `catalog`
- `actions`

### Providers

Provider 控制 Claude Code 和 Codex 如何连接模型 API。

```bash
arc provider list
arc provider use
arc provider use official --agent codex
arc provider test
```

规则：

- `arc provider` 等同于 `arc provider list`。
- 非交互式 `provider use` 必须提供 provider 名。
- 如果同名 provider 出现在多个 agent 中，需要传 `--agent`。
- Codex proxy provider 写入 Codex 原生配置时固定为 `name = "OpenAI"`；arc provider 名仍用于选择 profile。
- 只要任一被测试 provider 失败，`provider test` 就以 `1` 退出。

Provider 配置文件：

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

### Skills

列出和查看 skill：

```bash
arc skill list
arc skill info my-skill
arc skill list --format json
```

安装和卸载 skill：

```bash
arc skill install my-skill --agent claude
arc skill install my-skill --agent claude --agent codex
arc skill uninstall my-skill --agent claude
arc skill uninstall my-skill --all
```

全局 skill 路径：

| Agent | 路径 |
| --- | --- |
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>` |

项目级 skill 路径：

| Agent | 路径 |
| --- | --- |
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills-cursor/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

OpenClaw 的全局 skill 使用目录复制，不支持项目级 skill。

### Markets

Market 是包含 skill 的 git 仓库。

```bash
arc market list
arc market add https://github.com/team/skills.git
arc market update
arc market remove <git-url-or-id>
```

`arc market update` 会拉取 market、重建 catalog，并刷新 arc 跟踪的全局 skill 安装。它不会管理手工放进 agent 原生目录的文件。

跟踪元数据存储在：

```text
~/.arc-cli/state/skills/installs.json
```

如果跟踪元数据损坏，arc 会把它隔离为 `installs.corrupt.<unix_ts>.json`，然后以空跟踪状态继续。

### 项目配置

项目配置让仓库声明 provider、skill 和 market 要求。

常用命令：

```bash
arc project apply
arc project apply --agent codex
arc project apply --all-agents
arc project edit
```

`arc project apply` 会：

- 接入 `arc.toml` 中声明的 market；
- 切换到要求的 provider；
- 为选中的 agent 安装项目级 skill。

最小 `arc.toml`：

```toml
version = 1

[skills]
require = ["architecture-review"]
```

更完整示例：

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

规则：

- `arc.toml` 是项目配置入口。
- `arc project apply` 是真正修改本地状态的操作。
- `arc project edit` 会交互式编辑 skill requirements。
- `--agent` 和 `--all-agents` 选择项目级 skill 安装目标。
- `arc.toml` 不应包含 secret。
- `[mcps]` 和 `[subagents]` 已移除，出现时会按未知字段拒绝。

### Shell 补全

```bash
arc completion zsh
arc completion bash
arc completion fish
arc completion powershell
arc completion elvish
```

生成文件写入：

```text
~/.arc-cli/completions/
```

升级 `arc-kit` 后建议重新生成补全。

### 推荐工作流

个人配置：

```bash
arc status
arc provider use
arc skill list
arc skill install <name>
```

团队项目接入：

```bash
arc project apply
arc status
```

自动化：

```bash
arc status --format json
arc project apply --format json --agent codex
```

## 交互与自动化设计

本节定义面向人、脚本和 coding agent 的命令语义。

### 运行模式

`arc-kit` 只有两种运行模式：

| 模式 | 条件 |
| --- | --- |
| 交互式 | stdin 和 stdout 都是 TTY，且没有指定 `--format json` |
| 非交互式 | 没有 TTY，或指定了 `--format json` |

`--format json` 优先于 TTY 检测。即使命令在终端中运行，只要使用 `--format json`，就必须走自动化路径，不得启动 TUI 或 `dialoguer` 流程。

### JSON 与退出码

JSON 输出使用顶层 `schema_version`。当前 schema version：`"5"`。

`arc status --format json` 包含：

- `project`
- `agents`
- `catalog`
- `actions`

退出码约定：

| 场景 | 退出码 |
| --- | --- |
| 成功 | 0 |
| 配置解析失败 | 1 |
| `status` 报告 missing、partial 或 unavailable skills | 0 |
| 非交互式缺少必要参数 | 1 |
| `arc provider test` 有失败项 | 1 |
| JSON 序列化失败 | 1 |

写入类 JSON 在可预期且不执行变更的失败场景中可能以 `0` 退出并返回 `ok == false`，例如 `arc project apply --format json` 遇到缺失的 `arc.toml`。自动化必须检查 `ok` 和 `message`，不能只看进程退出码。

### JSON 覆盖

只读命令必须支持 `--format json`，除非明确登记为例外。

必须支持 JSON 的只读命令：

- `arc status`
- `arc market list`
- `arc skill list`
- `arc skill info <name>`
- `arc provider list`
- `arc provider test`
- `arc project edit` 的结构化失败结果

已登记例外：

- `arc version`
- 无子命令的裸 `arc`
- `arc completion`

JSON 输出不得包含 ANSI 转义序列。

### 写入命令

如果交互式命令提供向导、多选、确认或编辑器，非交互式路径必须由显式参数完成，并且不得读取 stdin。

当前一键路径：

| 命令 | 非交互式路径 |
| --- | --- |
| `skill install` / `skill uninstall` | 显式 name，并按需提供目标 agent 或 `--all` |
| `provider use` | 显式 provider name；存在歧义时提供 `--agent` |
| `market add` / `market remove` / `market update` | 由命令参数完整表达 |
| `project apply` | 项目 skill 需要安装时提供 `--agent` 或 `--all-agents` |
| `project edit` | 仅交互式编辑器；JSON 路径返回结构化失败，不打开编辑器 |

### 项目配置设计

`arc.toml` 支持：

- `version`
- `[provider]`
- `[[markets]]`
- `[skills]`

`[mcps]` 和 `[subagents]` 已移除，出现时会按未知字段拒绝。

当 `arc project apply` 在交互模式下运行且缺少 `arc.toml` 时，会打开项目 skill 编辑器创建配置。在非交互模式下缺少 `arc.toml` 时，纯文本路径以 `1` 退出；JSON 路径返回 `WriteResult.ok == false` 且退出码为 `0`。

### UI 边界

- 业务逻辑属于 `arc-core`。
- CLI 命令定义和用户输出属于 `arc-cli`。
- TUI 和 `dialoguer` 交互只属于 `arc-tui`。
- `arc-core` 不得向 stdout 输出，也不得依赖 UI 库。

列表型 TUI 必须按当前终端宽度裁剪每一行，不能依赖终端自动换行。

### 资源家族基线

当前唯一完整资源家族是 `skill`：

| 动词 | 交互式行为 | 非交互式行为 |
| --- | --- | --- |
| `list` | TTY browser，可 drill down 到详情 | 可 pipe 的文本和稳定 JSON 集合 |
| `info` | 从 list 进入详情，或直接查询 | 显式单项查询和稳定 JSON 详情 |
| `install` | 省略 name 时进入向导 | 显式 name 和目标 agent |
| `uninstall` | 省略 name 时从已安装项选择 | 显式 name 和目标 agent 或 `--all` |

新增其他资源家族时，需要同时评估 `list / info / install / uninstall` 是否支持人和 agent。

### 反例

- 只判断 TTY，忽略 `--format json`；
- JSON 中混入 ANSI；
- 在非交互模式调用 `dialoguer::Input::interact()`；
- 把文件系统或领域行为放进 `arc-cli`，而不是 `arc-core`；
- 新增只读命令但不提供 JSON 输出。

## 开发指南

### 环境

- Rust stable toolchain
- 目标平台为 macOS

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

### 必要检查

提交代码前：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

如果改动了 CLI 入口、输出格式或交互语义，还要执行：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

版本号变更、打 `v*` tag 或正式发布前：

```bash
./scripts/regression.sh
```

回归脚本会在隔离的 `ARC_KIT_USER_HOME` 下执行格式检查、构建、clippy、测试和 CLI 黑盒检查。

### 仓库结构

```text
.
├── arc-cli/          # CLI、clap 命令表、用户输出、JSON 结构体
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、detect、paths、io
├── arc-tui/          # 交互 UI；只有这个 crate 依赖 dialoguer
├── built-in/         # 内置 skill 和 market index
├── scripts/
│   └── regression.sh # 发版前回归
└── Cargo.toml
```

### 模块职责

- `arc-core`：业务逻辑、状态、文件系统操作、provider 应用、market 同步、skill registry、安装引擎、检测和项目解析。
- `arc-cli`：命令定义、命令分发、用户输出和 JSON 响应结构。
- `arc-tui`：交互式终端 UI、选择器、模糊浏览、向导流程和主题。

不要把业务逻辑放进 `arc-cli`。不要把 `dialoguer` 交互放进 `arc-core` 或 `arc-cli`。

### 文档要求

行为类代码变更必须更新相关 README 章节：

- 面向产品能力变化的内容；
- 用户工作流；
- CLI 语义、JSON 或交互变化；
- 构建、测试、发版或模块职责变化；
- 对应的 `README.zh-CN.md` 中文镜像内容。

代码注释和 CLI 提示使用英文。正式文档维护在 `README.md` 和 `README.zh-CN.md` 中。

### 贡献规则

- 每次变更保持范围聚焦。
- 行为变更必须带测试。
- 避免无关重构。
- 不引入未使用依赖。
- 持久化写入使用 `arc-core::io` 的原子写 helper。
- 终端布局和交互模式判断应靠近 CLI/TUI 边界。

### 发版规则

- 先确认 `main` push 成功，再推 release tag。
- 单独推送 tag。
- 不要执行 `git push origin main --tags`。

### 路线图备忘

- P0：provider、market 和 skill 行为必须保持稳定；改动需要测试。
- P1：加强 market/provider 黑盒和边界测试。
- P2：持续完善配置和 provider schema 行为文档。
