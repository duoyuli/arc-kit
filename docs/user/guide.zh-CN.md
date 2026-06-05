# arc-kit 用户指南

[English](guide.md)

`arc-kit` 是一个本地 CLI，用来管理 coding agent 的 provider、skill、market 和项目要求。

目标平台：macOS。

## 快速开始

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

## 交互模式

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

## 状态检查

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

## Providers

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
- 只要任一被测试 provider 失败，`provider test` 就以 `1` 退出。

Provider 配置文件：

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

## Skills

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

## Markets

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

## 项目配置

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

## Shell 补全

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

## 推荐工作流

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
