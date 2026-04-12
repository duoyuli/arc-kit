# arc-kit

> 编码 Agent 的配置管理工具

## 简介

如果你同时使用 Claude Code、Codex 等多个 Agent，你可能会遇到这样的麻烦：

- 切换 Agent 的模型供应商时，每个工具都要单独修改配置文件
- 写了一个好用的 Skill，想让所有助手都能用，得手动复制到不同目录
- Skill 升级后又得手动复制一遍
- 发现 GitHub 上有不错的 Skill 资源，clone 下来、找目录、复制粘贴
- 团队协作时，每个人的配置都不一样

**arc-kit 用一套配置解决这些问题。**

### 核心能力

**1. Provider 统一管理**

执行 `arc provider use <name> [--agent <agent>]` 切换 provider profile；若要按项目统一对齐 provider，可在 `arc.toml` 中声明 `[provider]` 后执行 `arc project apply`。自动备份，随时可回滚。支持官方直连、镜像代理、中转站、国内平替等。
其中 Codex 的 auth-only provider 会按 provider 名分别保存 `auth.json` 快照；切到中转站时 `auth.json` 会被重写为仅含 `OPENAI_API_KEY`，切回对应 auth provider 时再恢复该 provider 自己的登录态。

**2. Skill 一处管理，多处使用**

将 Skill 放入 `~/.arc-cli/skills/`，arc-kit 一键同步到所有已安装的 Agent 中。

三层来源，高优先级覆盖低优先级：

| 来源 | 路径 | 说明 |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 用户自定义 |
| market | 远程 git 仓库 | 社区或团队共享 |
| built-in | 嵌入 arc-kit 二进制 | 自带Skill，首次使用自动释放 |


**3. Market 发现与同步**

- 内置社区与官方优秀实践（Anthropic、OpenAI、MiniMax 等官方 Skill 资源）。
- 接入团队私有仓库，执行 `arc market add <Git 仓库地址>`
- 自动拉取更新，删除失效 Skill，重装变更路径的 Skill，执行 `arc market update`

**4. 项目级配置**

在仓库中放置 `arc.toml`，执行 `arc project apply`，即可让团队统一获得 Market、Skill、MCP 和 Subagent 配置，并让支持 provider 的 agent 对齐对应的 provider profile。该命令支持非交互式 `--format json` 输出，适合 CI/CD；首次执行且仓库内还没有 `arc.toml` 时，纯文本路径会报错，JSON 会返回结构化失败结果，便于自动化判断。

---

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

完整的项目级路径与能力支持矩阵见 [docs/user/guide.md](docs/user/guide.md)。

**Q: 为什么 OpenClaw 是特殊处理？**

OpenClaw 做了比较严格的安全检查，不支持软链接加载 Skill，因此使用目录复制，其余 agent 均使用软链接。

**Q: `arc market update` 会做什么？**

拉取所有 market 源的最新内容，重建索引。然后：
- 仅维护 **arc 已追踪** 的全局 skill 安装，不会删除手工放进 agent 目录的 skill
- 删除 registry 中已不存在的已追踪全局安装
- 仅在目标确实落后时才刷新（软链重指向、目录复制重写）

**Q: 项目级 skill 和全局 skill 有什么区别？**

全局 skill 安装在用户目录，对所有项目生效。项目级 skill 由 `arc.toml` 定义，`arc project apply` 安装到仓库内的 agent 路径（如 `.claude/skills/`、`.agents/skills/`），仅对当前项目生效。
> OpenClaw 不参与项目级安装。

**Q: MCP 和 Subagent 怎么管理？**

- 全局资源：`arc mcp install/uninstall`、`arc subagent install/uninstall`
- 项目资源：写入 `arc.toml`，执行 `arc project apply`
- 审计最终生效状态：`arc status` 或 `arc status --format json`

当前内置 MCP 预设包括 `filesystem`、`drawio`、`sequential-thinking`、`zhipu-web-search`。其中远程预设 `zhipu-web-search` 需要环境变量 `AUTHORIZATION_ZHIPU_WEB_SEARCH`。
当前内置 Subagent 包括 `arc-architecture`、`arc-backend`、`arc-brainstorm`、`arc-coordination`、`arc-db`、`arc-debug`、`arc-design`、`arc-dev-workflow`、`arc-frontend`、`arc-mobile`、`arc-orchestrator`、`arc-pdf`、`arc-pm`、`arc-qa`、`arc-scm`、`arc-tf-infra`、`arc-translator`，定义统一登记在 `built-in/subagent/index.toml`，可直接按名称安装。

项目级 MCP 默认不会写入仅支持全局 scope 的 agent；需要显式传 `--allow-global-fallback`，或在 `arc.toml` 里为该 MCP 声明 `scope_fallback = "global"`。**OpenClaw** 不参与 arc 管理的 MCP（`targets` 中不可选）。**Kimi CLI** 仅全局 MCP：默认 `~/.kimi/mcp.json`，若设置 `KIMI_SHARE_DIR` 则写该目录下的 `mcp.json`。
`--agent` 与 `targets = [...]` 只能填写受支持的 agent id；拼写错误会直接报错，不会静默跳过。
arc-kit 会把全局/项目 subagent 同步到已支持的 agent 原生目录。当前由 arc 写入并跟踪的 agent 为 **Claude Code**（`~/.claude/agents`、`./.claude/agents`）、**Codex**（`~/.codex/agents`、`.codex/agents`，TOML）、**OpenCode**（`~/.config/opencode/agents`、`.opencode/agents`，Markdown，含 `mode: subagent` frontmatter）。写入 **Codex** 时必须提供非空 `description`。

其他产品（如 **Gemini CLI** 的 `.gemini/agents`、**GitHub Copilot CLI** 的 `~/.copilot/agents` / `.github/agents`、**Windsurf** 按目录的 `AGENTS.md`）各有独立约定，**arc 当前不写入或管理**，需自行维护。若将不支持的 agent id 写入 `targets` 会直接校验失败。

**Q: 为什么只支持 macOS？**

因为我只有 macOS 电脑 😂

---


## 安装与使用

> 完整使用见用户手册

### Homebrew（推荐）

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

### 命令总览

```text
arc                     # 显示帮助
arc status              # 显示 Project / Agents / Catalog / MCPs / Subagents / Actions 状态
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
arc mcp list            # 列出全局 MCP
arc mcp install <name>  # 按预设名安装全局 MCP，或用 arc mcp define 写入自定义定义
arc mcp define <name>   # 新增或更新自定义全局 MCP
arc mcp uninstall <name>  # 卸载全局 MCP
arc mcp info <name>      # 显示 MCP 详情
arc subagent list       # 列出全局 subagent
arc subagent install    # 安装或更新全局 subagent（内置项可直接按名称安装）
arc subagent uninstall <name>  # 卸载全局 subagent
arc subagent info <name>      # 显示 subagent 详情
arc project apply       # 应用 arc.toml 配置（支持 --agent / --all-agents）
arc project edit        # 编辑 arc.toml（交互式）
```

交互式列表类界面会按当前终端视口宽度裁剪显示，避免窄窗口下因自动换行造成重绘残影。


## 文档

| 文档 | 内容 |
|------|------|
| [docs/user/guide.md](docs/user/guide.md) | 完整用户手册 |
| [docs/developer/design.md](docs/developer/design.md) | 交互/非交互设计规范 |
| [docs/developer/development.md](docs/developer/development.md) | 开发贡献指南 |
