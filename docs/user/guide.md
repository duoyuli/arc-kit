# arc-kit 产品使用说明书

## 1. 产品简介

`arc-kit` 是一个给 coding agent 用的本地配置管理工具，当前管理三类内容：

- Provider：切换 Claude Code / Codex 当前使用的 provider
- Skill：给 agent 安装能力说明
- Market：接入和同步 skill 仓库

项目配置通过 `arc.toml` 把 provider、skills 和 markets 写进仓库，然后用 `arc project apply` 落地。

> MCP 与 subagent 管理功能已移除；相关命令和 `arc.toml` section 不再可用。

当前目标平台：`macOS`。

## 2. 五分钟上手

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

验证环境：

```bash
arc --help
arc version
arc status
```

安装 skill：

```bash
arc market add https://github.com/example/skills.git
arc market update
arc skill install my-skill --agent claude --agent codex
```

应用到项目：

```bash
arc project apply
arc status
```

如果当前项目里没有 `arc.toml`，交互式执行 `arc project apply` 时会先帮你创建。

## 3. 交互式与自动化

直接在终端里执行命令会进入面向人的交互界面，例如：

```bash
arc provider use
arc skill install
arc project apply
```

加上 `--format json` 会走自动化路径，不进入交互界面：

```bash
arc status --format json
arc project apply --format json --agent codex
```

## 4. 状态检查

`arc status` 用来看当前环境是不是已经准备好。

它会告诉你：

- 当前有没有识别到 agent
- 当前项目有没有 `arc.toml`
- 项目要求的 skill 有没有落地
- provider 是否和项目要求一致
- 下一步建议做什么

常用命令：

```bash
arc status
arc status --format json
```

JSON 顶层模块包括：

- `project`
- `agents`
- `catalog`
- `actions`

## 5. Provider 使用

Provider 用来切换 Claude Code 和 Codex 当前使用的模型接入方式。

```bash
arc provider list
arc provider use
arc provider use official --agent codex
arc provider test
```

规则摘要：

- `arc provider` 等同于 `arc provider list`
- 非交互式下，`use` 必须显式写 provider 名
- 如果同名 provider 出现在多个 agent，需要加 `--agent`
- `provider test` 只要有一项失败，退出码就是 `1`

Provider 配置文件在：

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

## 6. Skill 使用

查看 skill：

```bash
arc skill list
arc skill info my-skill
arc skill list --format json
```

安装 skill：

```bash
arc skill install my-skill --agent claude
arc skill install my-skill --agent claude --agent codex
```

卸载 skill：

```bash
arc skill uninstall my-skill --agent claude
arc skill uninstall my-skill --all
```

全局 skill 路径：

| Agent | 路径 |
|------|------|
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>` |

项目级 skill 路径：

| Agent | 路径 |
|------|------|
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

OpenClaw 使用目录复制，不支持项目级 skill。

## 7. Market 使用

Market 是 skill 来源仓库。

```bash
arc market list
arc market add https://github.com/team/skills.git
arc market update
arc market remove <git-url-or-id>
```

`arc market update` 会拉取所有 market，重建 catalog，并刷新 arc 已追踪的全局 skill 安装。

## 8. 项目配置与 arc.toml

项目功能用来把一组要求写进仓库，然后一键应用到当前项目。

常用命令：

```bash
arc project apply
arc project apply --agent codex
arc project apply --all-agents
arc project edit
```

`project apply` 会：

- 自动接入 `arc.toml` 中声明的 market
- 自动切换项目要求的 provider
- 自动安装项目级 skill

最简单的 `arc.toml`：

```toml
version = 1

[skills]
require = ["architecture-review"]
```

常用例子：

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]
```

规则摘要：

- `arc.toml` 是项目配置入口
- `project apply` 是真正落地
- `project edit` 是交互式修改 skill require
- `--agent` / `--all-agents` 影响项目级 skill 安装目标
- `arc.toml` 不保存 secret
- `[mcps]` 与 `[subagents]` 已移除，出现时会被当作未知字段拒绝

## 9. Shell 补全

```bash
arc completion zsh
arc completion bash
arc completion fish
arc completion powershell
arc completion elvish
```

生成文件会写到：

```text
~/.arc-cli/completions/
```

升级 `arc-kit` 后，建议重新执行一次补全生成命令。

## 10. 推荐使用路径

个人使用：

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

自动化 / Agent 场景：

```bash
arc status --format json
arc project apply --format json --agent codex
```
