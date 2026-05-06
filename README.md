# arc-kit

> 编码 Agent 的 provider、skill 与 market 配置管理工具

## 简介

在同时使用 Claude Code、Codex 等多个 Agent 时，常见问题包括：

- 切换模型供应商时，每个工具都要单独修改配置文件
- 写了一个好用的 Skill，想让所有助手都能用，得手动复制到不同目录
- Skill 升级后又得手动复制一遍
- 接入 GitHub 上的 Skill 仓库时，需要手动 clone、定位目录并复制
- 团队协作时，每个人的配置都不一样

**arc-kit 用同一套 CLI 管理 provider、skill、market 和项目级 skill 落地。**

## 核心能力

**1. Provider 统一管理**

执行 `arc provider use <name> [--agent <agent>]` 可切换 provider profile。交互式模式按 coding agent 分 tab 展示，一次只看一个 agent 的 provider，支持方向键与 `h/j/k/l` 导航、`q` 退出。若需在项目内固定 provider，可在 `arc.toml` 中声明 `[provider]` 后执行 `arc project apply`。

**2. Skill 一处管理，多处使用**

本地 skill 目录为 `~/.arc-cli/skills/`。加入 catalog 后，可通过 `arc skill install <name>` 安装到目标 agent。

三层来源，高优先级覆盖低优先级：

| 来源 | 路径 | 说明 |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 用户自定义 |
| market | 远程 git 仓库 | 社区或团队共享 |
| built-in | 嵌入 arc-kit 二进制 | 自带 Skill，首次使用自动释放 |

**3. Market 发现与同步**

- 可接入官方或社区维护的 skill 仓库
- 可接入团队私有仓库：`arc market add <Git 仓库地址>`
- 拉取更新并刷新 catalog：`arc market update`

**4. 项目级配置**

在仓库中放置 `arc.toml` 后，执行 `arc project apply` 可同步 market、skill 和 provider 要求。该命令支持非交互式 `--format json` 输出，适用于 CI/CD；首次执行且仓库内还没有 `arc.toml` 时，交互式路径会先进入单屏 `Project Skills` 编辑器创建配置，非交互式纯文本会报错，JSON 会返回结构化失败结果。

> MCP 与 subagent 管理功能已移除；`arc.toml` 只接受 `provider`、`skills`、`markets` 和 `version`。

## FAQ

**Q: arc-kit 支持哪些 Agent？**

当前支持 Claude Code、Codex、Cursor CLI、OpenClaw、OpenCode、Gemini CLI、Kimi CLI。安装时自动检测已安装的 agent。

**Q: 安装 Skill 后，各个 Agent 的目录结构会是什么样？**

默认使用软链接安装（OpenClaw 除外，使用目录复制）：

| Agent | 全局 Skill 路径 |
|------|------|
| Claude Code | `~/.claude/skills/<name>` |
| Codex | `~/.codex/skills/<name>` |
| Cursor CLI | `~/.cursor/skills-cursor/<name>` |
| OpenCode | `~/.config/opencode/skills/<name>` |
| Gemini CLI | `~/.gemini/skills/<name>` |
| Kimi CLI | `~/.kimi/skills/<name>` |
| OpenClaw | `~/.openclaw/skills/<name>`（目录复制） |

项目级 skill 由 `arc.toml` 定义，`arc project apply` 安装到仓库内的 agent 路径：

| Agent | 项目级 Skill 路径 |
|------|------|
| Claude Code | `./.claude/skills/<name>` |
| Codex | `./.codex/skills/<name>` |
| Cursor CLI | `./.cursor/skills/<name>` |
| OpenCode | `./.opencode/skills/<name>` |
| Gemini CLI | `./.gemini/skills/<name>` |
| Kimi CLI | `./.kimi/skills/<name>` |

> OpenClaw 不参与项目级安装。

**Q: `arc market update` 会做什么？**

拉取所有 market 源的最新内容，重建索引。然后仅维护 **arc 已追踪** 的全局 skill 安装，不会删除手工放进 agent 目录的 skill。

## 安装与使用

### Homebrew（推荐）

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

### 命令总览

```text
arc                     # 显示帮助
arc status              # 显示 Project / Agents / Catalog / Actions 状态
arc version             # 显示版本（无 --format json）
arc completion <shell>  # 生成 shell 补全
arc provider list       # 列出可用模型供应商
arc provider use        # 切换模型供应商
arc provider test       # 测试模型供应商连通性
arc market list         # 列出 market 源
arc market add <url>    # 添加 market 源
arc market remove <git-url-or-id>  # 移除 market 源
arc market update       # 更新所有 market 源
arc skill list          # 列出 skills
arc skill install       # 安装 skill
arc skill uninstall     # 卸载 skill
arc skill info          # 显示 skill 详情
arc project apply       # 应用 arc.toml 配置
arc project edit        # 交互式编辑 arc.toml skills
```

`arc project edit` / 首次执行的 `arc project apply` 使用同一套单屏 skill 编辑器：可直接搜索 skill，`space` 勾选，`enter` 保存，`esc` 取消且不写文件。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/user/guide.md](docs/user/guide.md) | 产品使用说明书 |
| [docs/developer/design.md](docs/developer/design.md) | 交互/非交互设计规范 |
| [docs/developer/development.md](docs/developer/development.md) | 开发贡献指南 |
