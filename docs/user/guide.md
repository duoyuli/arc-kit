# arc-kit 产品使用说明书

## 1. 产品简介

`arc-kit` 是一个给 coding agent 用的本地配置管理工具。

它主要解决这几类问题：

- Claude Code、Codex 等多个 agent 的配置不统一
- skill、MCP、subagent 需要反复手工复制
- 团队项目希望把要求写进仓库，一键同步

当前目标平台：`macOS`

## 2. 它能做什么

`arc-kit` 主要管理五类内容：

- Provider：切换 Claude Code / Codex 当前使用的 provider
- Skill：给 agent 安装能力说明
- MCP：给 agent 接入外部工具或服务
- Subagent：给 agent 安装角色说明
- 项目配置：通过 `arc.toml` 把项目要求写进仓库

## 3. 五分钟上手

### 安装

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

### 验证环境

```bash
arc --help
arc version
arc status
```

### 切换 Provider

```bash
arc provider list
arc provider use
arc provider test
```

### 安装能力

```bash
arc market add https://github.com/example/skills.git
arc market update

arc skill install my-skill --agent claude --agent codex
arc mcp install filesystem --agent codex
arc subagent install arc-backend --agent claude
```

### 应用到项目

```bash
arc project apply
arc status
```

如果当前项目里没有 `arc.toml`，交互式执行 `arc project apply` 时会先帮你创建。

## 4. 基础规则

### 裸命令

```bash
arc
```

等同于：

```bash
arc --help
```

### 交互式

直接在终端里执行命令，例如：

```bash
arc provider use
arc skill install
arc project apply
```

这时会进入面向人的交互界面。

### 自动化 / Agent 使用

加上：

```bash
--format json
```

例如：

```bash
arc status --format json
arc project apply --format json --agent codex
```

这时不会进入交互界面，适合脚本、CI、Agent。

## 5. 状态检查

### 这个功能是干什么的

`arc status` 用来看当前环境是不是已经准备好了。

它会告诉你：

- 当前有没有识别到 agent
- 当前项目有没有 `arc.toml`
- 项目要求的 skill / MCP / subagent 有没有落地
- 下一步建议做什么

### 最常用命令

```bash
arc status
arc status --format json
```

### 什么时候用

- 刚安装完 `arc-kit`
- 刚切换完 provider
- 刚执行完 `arc project apply`
- 怀疑某个配置没有真正生效

### 最常看的两块

- `Project`
- `Actions`

如果这里没有问题，说明整体通常已经可用。

## 6. Provider 使用

### 这个功能是干什么的

Provider 用来切换 Claude Code 和 Codex 当前使用的模型接入方式。

### 最常用命令

```bash
arc provider list
arc provider use
arc provider use proxy --agent claude
arc provider test
arc provider test --agent codex
```

### 常见用法

先看有哪些 provider：

```bash
arc provider list
```

交互式切换：

```bash
arc provider use
```

交互界面支持 `↑↓` 或 `j/k` 移动，`←→` / `tab` 或 `h/l` 切换 agent，`enter` 选择，`esc` 或 `q` 退出。

指定切换：

```bash
arc provider use official --agent codex
```

测试连通性：

```bash
arc provider test
```

### 规则摘要

- Provider 目前只支持 Claude Code 和 Codex
- `arc provider` 等同于 `arc provider list`
- 非交互式下，`use` 必须显式写 provider 名
- 如果同名 provider 出现在多个 agent，需要加 `--agent`
- `provider test` 只要有一项失败，退出码就是 `1`

### Provider 怎么配

Provider 配置文件在本机用户目录下：

```text
~/.arc-cli/providers/claude.toml
~/.arc-cli/providers/codex.toml
```

规则很简单：

- 每个 `[section]` 就是一个可切换的 provider profile
- section 名就是你执行 `arc provider use <name>` 时用的名字
- 这些文件是本机配置，不是项目配置，不要提交到仓库

若对应 agent 已检测到，`arc-kit` 首次运行时通常会自动生成一个默认的 `official` profile；也可直接手工编辑。

### Claude Code 怎么配

Claude 的配置文件是：

```text
~/.arc-cli/providers/claude.toml
```

写法示例：

```toml
[official]
display_name = "Anthropic"
description = "Anthropic 官方订阅"

[my-proxy]
display_name = "My Proxy"
description = "通过中转站访问 Anthropic API"
ANTHROPIC_BASE_URL = "https://your-proxy.example.com"
ANTHROPIC_AUTH_TOKEN = "sk-ant-xxx"
ANTHROPIC_DEFAULT_OPUS_MODEL = ""
ANTHROPIC_DEFAULT_SONNET_MODEL = ""
ANTHROPIC_DEFAULT_HAIKU_MODEL = ""
```

规则摘要：

- `display_name` 和 `description` 是展示信息
- 其余字段会被当成环境变量写给 Claude Code
- 常见场景就是配 `ANTHROPIC_BASE_URL` 和 `ANTHROPIC_AUTH_TOKEN`

写完后执行：

```bash
arc provider list
arc provider use my-proxy --agent claude
arc provider test --agent claude
```

### Codex 怎么配

Codex 的配置文件是：

```text
~/.arc-cli/providers/codex.toml
```

Codex 有两种 profile：

- `auth-only`：只有显示信息，不写代理地址
- `proxy`：必须同时写 `base_url` 和 `api_key`

写法示例：

```toml
[official]
display_name = "OpenAI"
description = "OpenAI 官方订阅"

[my-proxy]
display_name = "My Proxy"
description = "通过中转站访问 OpenAI API"
base_url = "https://your-proxy.example.com"
api_key = "sk-xxx"
```

规则摘要：

- 只有 `display_name` / `description` 的 profile，会被当成 `auth-only`
- 如果写了 `base_url`，就必须同时写 `api_key`
- `proxy` 切换时会把 Codex 当前认证改成对应的代理配置

写完后执行：

```bash
arc provider list
arc provider use my-proxy --agent codex
arc provider test --agent codex
```

### 配完以后怎么生效

推荐固定按这三步：

```bash
arc provider list
arc provider use <name> --agent <agent>
arc provider test --agent <agent>
```

如果 `provider test` 成功，说明这个 provider 基本已经可用。

### 项目统一

如需让某个项目固定使用某个 provider，可将其写入 `arc.toml`，然后执行：

```bash
arc project apply
```

## 7. Market 使用

### 这个功能是干什么的

Market 用来接入资源仓库。

常见场景：

- 接入团队私有 skill 仓库
- 接入社区资源仓库
- 更新本地 catalog

### 最常用命令

```bash
arc market list
arc market add https://github.com/example/skills.git
arc market update
arc market remove <git-url-or-source-id>
```

### 常见用法

查看已接入的 market：

```bash
arc market list
```

新增一个 market：

```bash
arc market add https://github.com/example/skills.git
```

更新所有 market：

```bash
arc market update
```

### 规则摘要

- `arc market` 等同于 `arc market list`
- `market add` 后会自动扫描资源
- `market update` 会刷新所有 market 并更新本地 catalog
- `market remove` 不会自动卸载你已经装到 agent 里的内容

### 和项目的关系

如果 `arc.toml` 里写了 `[[markets]]`，执行：

```bash
arc project apply
```

时会自动把缺失 market 加到本地。

## 8. Skill 使用

### 这个功能是干什么的

Skill 用来给 agent 增加能力。

### 最常用命令

```bash
arc skill list
arc skill info <name>
arc skill install <name>
arc skill uninstall <name>
```

### 常见用法

查看 skill：

```bash
arc skill list
arc skill list --installed
```

看某个 skill：

```bash
arc skill info architecture-review
```

安装 skill：

```bash
arc skill install my-skill
arc skill install my-skill --agent claude --agent codex
```

卸载 skill：

```bash
arc skill uninstall my-skill
arc skill uninstall my-skill --all
```

### 规则摘要

- `arc skill` 等同于 `arc skill list`
- 非交互式下，`install` 和 `uninstall` 必须写 skill 名
- 不传 `--agent` 时，默认装到所有支持 skill 的已检测 agent

### 全局和项目的区别

全局 skill：

```bash
arc skill install ...
```

项目 skill：

1. 写进 `arc.toml`
2. 执行：

```bash
arc project apply
```

项目级 skill 会按 agent 的仓库内原生目录写入，例如 Claude Code 为 `./.claude/skills/<name>`，Codex 为 `./codex/skills/<name>`。

## 9. MCP 使用

### 这个功能是干什么的

MCP 用来给 agent 接入外部能力，例如：

- 文件系统
- 图表工具
- 联网搜索
- 自定义服务

### 最常用命令

```bash
arc mcp list
arc mcp info filesystem
arc mcp install filesystem --agent codex
arc mcp define mysvc --transport stdio --command npx --arg -y --arg @scope/pkg
arc mcp uninstall filesystem
```

### 常见用法

查看有哪些 MCP：

```bash
arc mcp list
```

查看某个 MCP：

```bash
arc mcp info filesystem
```

安装内置 MCP：

```bash
arc mcp install filesystem --agent codex
```

定义自定义 MCP：

```bash
arc mcp define mysvc \
  --transport stdio \
  --command npx \
  --arg -y \
  --arg @scope/pkg
```

### 当前常用内置 MCP

- `filesystem`
- `drawio`
- `sequential-thinking`
- `zhipu-web-search`

### 规则摘要

- `arc mcp` 等同于 `arc mcp list`
- 非交互式下，`info` / `install` / `uninstall` 都要显式写名称
- `stdio` 类型用 `--command`
- 远程类型用 `--url`
- token 不要明文写，改用环境变量

### 特别提醒

使用 `zhipu-web-search` 之前，先准备：

```bash
export AUTHORIZATION_ZHIPU_WEB_SEARCH=...
```

### 和项目的关系

如果某个项目需要固定 MCP：

1. 先让这些 MCP 名称在全局可用
2. 在 `arc.toml` 的 `[mcps] require` 里写名称
3. 执行：

```bash
arc project apply
```

Kimi CLI 仅支持全局 MCP。项目级声明会在 `arc project apply` 中被跳过；如需让 Kimi 使用某个 MCP，应执行全局 `arc mcp install <name> --agent kimi`。

## 10. Subagent 使用

### 这个功能是干什么的

Subagent 用来给支持原生 subagent 的 agent 安装角色说明。

### 最常用命令

```bash
arc subagent list
arc subagent info arc-backend
arc subagent install arc-backend --agent claude
arc subagent install reviewer --prompt-file ./reviewer.md --description "Code review helper" --agent codex
arc subagent uninstall reviewer
```

### 常见用法

查看有哪些 subagent：

```bash
arc subagent list
```

查看某个 subagent：

```bash
arc subagent info arc-brainstorm
```

安装内置 subagent：

```bash
arc subagent install arc-backend --agent claude
```

安装自定义 subagent：

```bash
arc subagent install reviewer \
  --prompt-file ./reviewer.md \
  --description "Code review helper" \
  --agent codex
```

### 当前主要支持的目标

- Claude Code
- Codex
- OpenCode

### 规则摘要

- `arc subagent` 等同于 `arc subagent list`
- 非交互式下，`info` / `install` / `uninstall` 都要显式写名称
- 自定义 subagent 一般要提供 `--prompt-file`
- 如果目标包含 Codex，必须提供非空 `description`

### 和项目的关系

如果项目要固定使用某个 subagent：

1. 先让这个 subagent 在全局可用
2. 在 `arc.toml` 的 `[subagents] require` 里写名称
3. 执行：

```bash
arc project apply
```

## 11. 项目配置与 `arc.toml`

### 这个功能是干什么的

项目功能用来把一组要求写进仓库，然后一键应用到当前项目。

核心文件：

```text
arc.toml
```

### 最常用命令

```bash
arc project apply
arc project apply --agent codex
arc project apply --all-agents
arc project edit
```

### `project apply`

作用很简单：

- 自动接入需要的 market
- 自动切换需要的 provider
- 自动安装项目级 skill
- 自动写入项目级 MCP 和 subagent

执行前的纯文本预览会按分段列出 `arc.toml` 中要求的 `provider`、`skills`、`mcps` 和 `subagents`，并标明如 `present`、`will install`、`will apply`、`not in catalog` 之类的状态。

第一次在项目中使用时，如果还没有 `arc.toml`，交互式执行：

```bash
arc project apply
```

会先帮你创建。

### `project edit`

用于交互式编辑项目要求：

```bash
arc project edit
```

改完后通常还要再执行一次：

```bash
arc project apply
```

### 最简单的 `arc.toml`

```toml
version = 1

[skills]
require = ["architecture-review"]
```

### 一个常用例子

```toml
version = 1

[provider]
name = "official"

[[markets]]
url = "https://github.com/team/skills.git"

[skills]
require = ["team-review"]

[mcps]
require = ["filesystem"]

[subagents]
require = ["arc-qa"]
```

### 规则摘要

- `arc.toml` 是项目配置入口
- `project apply` 是真正落地
- `project edit` 是改配置
- `--agent` / `--all-agents` 主要影响项目级 skill 安装目标
- secret 不写进 `arc.toml`
- MCP 和 subagent 在项目里只写名称，不写完整定义

## 12. Shell 补全

如需开启命令补全：

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

## 13. 推荐使用路径

### 个人使用

```bash
arc status
arc provider use
arc skill list
arc skill install <name>
```

### 团队项目接入

```bash
arc project apply
arc status
```

### 自动化 / Agent 场景

```bash
arc status --format json
arc project apply --format json --agent codex
```

## 14. 常见问题

### 我只想先确认环境有没有问题

先执行：

```bash
arc status
```

### 我只想给本机增加能力

按类型选一个命令：

```bash
arc skill install ...
arc mcp install ...
arc subagent install ...
```

### 我想把团队项目统一起来

直接在项目根目录执行：

```bash
arc project apply
```

### 我想给脚本或 Agent 用

优先使用支持 `--format json` 的命令。

已登记例外：

- `arc version` 不支持 JSON 输出
- `arc project edit --format json` 只返回失败结果，不会执行编辑
