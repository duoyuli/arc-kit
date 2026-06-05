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

## 文档

| 文档 | 用途 |
| --- | --- |
| [docs/user/guide.zh-CN.md](docs/user/guide.zh-CN.md) | 完整用户手册 |
| [docs/developer/design.zh-CN.md](docs/developer/design.zh-CN.md) | 交互/非交互与 JSON 设计规则 |
| [docs/developer/development.zh-CN.md](docs/developer/development.zh-CN.md) | 开发流程、门禁和仓库结构 |
| [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md) | 贡献入口 |

英文默认文件使用相同路径，不带 `.zh-CN` 后缀。
