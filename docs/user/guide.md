# 用户手册

安装、命令与项目配置的一站式说明。交互与自动化设计原则见 [交互与自动化设计](../developer/design.md)。

---

## 安装

### Homebrew（推荐）

```bash
brew tap duoyuli/arc-kit https://github.com/duoyuli/arc-kit.git
brew install arc-kit
```

自动处理架构（Apple Silicon / Intel）与 PATH。升级：`brew upgrade arc-kit`。

### 从源码构建

**前置**：Rust（`rustc` / `cargo`），[rustup](https://rustup.rs/)；**平台**：当前以 macOS 为主。

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo install --path arc-cli --force
```

若提示找不到 `arc`，将 Cargo bin 加入 PATH（可写入 `~/.zshrc`）：

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### 验证安装

```bash
arc --help
arc version
```

有输出即表示可执行文件可用。

---

## 全局选项

查看完整帮助：

```bash
arc --help
arc <command> --help
```

| 选项 | 说明 |
|------|------|
| `-v, --verbose` | 详细日志：在未设置 `ARC_LOG` 时默认 `debug`（日志写入 `~/.arc-cli/arc.log`） |
| `--format <FORMAT>` | `text`（默认）或 `json`。`json` 时输出带 `schema_version` 的稳定 JSON，不混入 ANSI |

### 交互式与非交互式

CLI 只分两类语义（详见 [交互与自动化设计](../developer/design.md)）：

- **交互式**：标准输入、标准输出均为 TTY，且未使用 `--format json` → 面向人，可有 TUI、彩色、确认框。
- **非交互式**：管道/重定向/CI，或使用了 `--format json` → 面向脚本与 Agent；默认 `text` 时便于 pipe，`json` 时输出稳定 JSON。

`--format json` 优先于 TTY：在交互终端里也可以请求 JSON，此时不按交互式 UI 渲染。

除 `completion` 与已登记例外外，主要命令均支持 `--format json`：

- **读取类**：`status`、`skill list`、`skill info`、`mcp list`、`mcp info`、`subagent list`、`subagent info`、`provider list`、`market list`、`provider test`
- **写入类**：`skill install`、`skill uninstall`、`mcp install`、`mcp uninstall`、`subagent install`、`subagent uninstall`、`provider use`、`market add`、`market remove`、`market update`、`project apply`、`project edit`（*`project edit` 仅交互式；JSON 下返回 `ok: false` 错误，不执行编辑*）

**约定**：只读命令须实现 `--format json`（例外见 [交互与自动化设计](../developer/design.md)，如 **`arc version`**）。带向导的写入命令须在**非交互式**下提供**显式参数**，以便一键执行、不读 stdin。写入类 JSON 使用统一 `WriteResult`（`ok` / `message` / `items`）。`arc project apply` / `project edit` 的边界情况见下文 [project apply](#project-apply) / [project edit](#project-edit)。

设计与实现对照见 [交互与自动化设计](../developer/design.md)。

---

## status

查看当前状态快照，固定分成六个模块：

- `Project`：当前仓库的 `arc.toml`、required skills 落地进度、项目级 provider 对齐情况
- `Agents`：当前机器检测到的 agent、版本、active provider、全局 skill 数
- `Catalog`：market 数量、resource 总量、全局 skill 总量
- `MCPs`：全局/项目 MCP 的 rollout、scope 与 drift
- `Subagents`：全局/项目 subagent 的 rollout 与 drift
- `Actions`：建议下一步命令

若存在 `arc.toml`，从**当前工作目录向上**查找最近的配置文件（子目录可继承上级项目）。

裸调用 `arc`（无子命令）时打印 **`arc --help`**，不执行 `status`。

```bash
arc status
arc --help   # 与裸 `arc` 输出相同
```

`arc status` 为只读：**不**执行安装。`Project` 模块里，required skill 会被归类为：

- `ready`：已在所有已检测且支持项目级 skills 的 agent 路径下落地
- `partial`：仅在部分此类 agent 路径下落地
- `missing`：catalog 中存在，但在这些项目路径下尚未落地
- `unavailable`：当前 catalog 中根本找不到该 skill

若 `arc.toml` 声明了 `[provider] name`，`Project` 模块还会显示各 provider-capable agent 是否已对齐；若未对齐，会提示下一步命令。

---

## version

```bash
arc version
```

仅打印文本版本号（`arc v…`），**不使用** `--format json`（在初始化状态目录之前即返回）。

---

## provider

管理和切换 LLM provider profile。配置在 `~/.arc-cli/providers/<agent>.toml`。

```bash
arc provider list
arc provider          # 等同于 arc provider list

arc provider use                        # 交互式下选择
arc provider use proxy --agent claude
arc provider use openai --agent codex

arc provider test                       # 测试所有激活的 provider
arc provider test <name>
arc provider test --agent claude
```

当前支持的 agent：`claude`、`codex`。

`arc provider test` 会向 API 端点探测连通性；任一受测项失败则**退出码 1**（含 `--format json`）；请同时查看 JSON 中每项的 `ok`。

`arc provider use <name>` 只会切换**一个解析后的 provider profile**。若同名 profile 同时存在于多个 agent（例如 `official` 同时存在于 Claude 与 Codex），命令会报歧义并要求显式传 `--agent`。若要按项目对齐 provider，使用 `[provider]` + `arc project apply`。

- **Claude** — profile 中除 `display_name` / `description` 外的字段写入 `~/.claude/settings.json` 的 `env`；切换时清除上一 provider 的变量。
- **Codex** — provider 只支持两类：`auth-only`（仅 `display_name` / `description`）与 `proxy`（必须同时提供 `base_url` + `api_key`）。切到 `proxy` 时，`~/.codex/auth.json` 会被重写为仅含 `OPENAI_API_KEY`；切离 `auth-only` 时，当前登录态会按 provider 名保存到 `~/.arc-cli/backups/state/providers/codex/`，后续切回该 provider 时恢复其自己的快照。首次切到尚未登录过的 `auth-only` provider 时，会清空当前 `auth.json`，进入未登录状态。
- **Backups** — 切换 provider 等写操作前，相关文件会备份到 `~/.arc-cli/backups/<年>/<月>/<日>/`；本地日期早于「今天 − 60 天」的会话目录会在后续备份时清理。

---

## market

管理 market 源。裸调用 `arc market` 等同于 `arc market list`。

```bash
arc market add https://github.com/example/skills.git
arc market list
arc market        # 等同于 arc market list
arc market remove <git-url-or-source-id>   # 内置源（built-in）不可移除
arc market update   # pull、重建 catalog，并维护全局 skill 安装（见下）
```

`arc market update` 完成 catalog 重建后，会按**合并后的 registry**（`local` > `built-in` > `market`）自动维护 **arc 已追踪** 的全局安装：① **删除** registry 中**已不存在**的已追踪 skill 安装（软链或 OpenClaw 目录复制）；② 仅当目标**确实落后于当前 `resolve_source_path`** 时才重写安装，使其指向最新路径（远程只改目录结构时会更新软链/复制内容；手工放进 agent 目录但未被 arc 追踪的 skill 不会被误删）。

---

## skill

列出、安装、卸载、查看 skill。来源：`local` > `built-in` > `market`，同名按优先级去重。裸调用 `arc skill` 等同于 `arc skill list`。

交互式浏览列表时，长行会按当前终端宽度裁剪显示；在窄窗口中不会依赖终端自动换行。

```bash
arc skill list
arc skill list --installed
arc skill install
arc skill install my-skill
arc skill install my-skill --agent claude --agent codex
arc skill uninstall my-skill --all
arc skill uninstall my-skill --agent claude
arc skill info my-skill
```

### Skill 来源

| 来源 | 路径 | 说明 |
|------|------|------|
| local | `~/.arc-cli/skills/<name>/` | 用户自定义，含 `SKILL.md` |
| built-in | 嵌入二进制 | 首次使用释放到缓存 |
| market | `~/.arc-cli/markets/repo/...` | `arc market add` 添加的仓库 |

默认软链接安装到各 agent；**OpenClaw** 为目录复制到 `~/.openclaw/skills/<name>/`（其余 agent 均为软链接）。**Cursor** 的全局安装目录为 `~/.cursor/skills-cursor/<name>/`（项目级路径仍为 `.cursor/skills/<name>/`）。

**全局维护**（`arc market update` 在重建 catalog 之后自动执行）：先按上表合并 registry **删掉**已消失的、且由 arc 追踪的全局安装，再对仍登记且目标已落后的安装做 **刷新**，以指向当前解析路径（含 market 仓库内路径变更）。手工安装内容不在自动维护范围内。

### 项目与全局

`arc.toml` 中 `[skills] require` 为本仓库声明。`arc project apply` 会把 skill 落到**仓库内**下列路径（仅针对你选定的、已检测且支持项目级的 agent）：**交互式**下若确有缺失 skill 可装，会多选目标 agent；**非交互式**下须显式传 **`--agent <id>`**（可重复）或 **`--all-agents`**（装到当前检测到的全部此类 agent）。`arc skill install` 则为**全局**安装到 `~` 下各 agent 目录。`arc status` 与 `arc project apply` 的列表：前者区分「仓库内完全未出现」与「尚未在所有相关 agent 路径下复制」；后者在未全部落地时仍会尝试安装，直至每个支持项目级的已检测 agent 路径下都有对应 skill（或你仅选择部分 agent 时只写入选中目标）。

| Agent | 仓库内项目级 skill 路径（`<repo>/…`） |
|-------|----------------------------------------|
| Claude | `.claude/skills/<name>/` |
| Codex | `.agents/skills/<name>/` |
| Cursor | `.cursor/skills/<name>/` |
| OpenCode | `.opencode/skills/<name>/` |
| Gemini | `.gemini/skills/<name>/` |
| Kimi | `.kimi/skills/<name>/` |
| OpenClaw | **不支持**（不参与 `arc project apply` 的项目级安装；请用 `arc skill install` 装到 `~/.openclaw/skills`） |

---

## mcp

管理全局 MCP 定义。裸调用 `arc mcp` 等同于 `arc mcp list`。

```bash
arc mcp list
arc mcp info github

arc mcp install github --transport stdio --command npx --arg @modelcontextprotocol/server-github
arc mcp install github --agent claude --agent cursor --transport streamable-http --url https://example.com/mcp
arc mcp uninstall github
```

- `arc mcp install` 只管理**全局**定义；项目级 MCP 请写入 `arc.toml` 的 `[[mcps]]`，再执行 `arc project apply`。
- `--agent` 可重复；不传时表示写入所有支持该资源类型的 agent。
- CLI 参数中的 transport 取值为 `stdio`、`sse`、`streamable-http`；写入 `arc.toml` 时使用 TOML 枚举值 `stdio`、`sse`、`streamable_http`。
- `stdio` transport 必须提供 `--command`，不能提供 `--url`；远程 transport（`sse` / `streamable-http`）必须提供 `--url`，不能提供 `--command`。
- `--env KEY=VALUE` 与 `--header KEY=VALUE` 支持重复传入；涉及 secret 的键（如 `token`、`authorization`、`api_key`）必须使用环境变量占位符，例如 `${GITHUB_TOKEN}` 或 `Bearer ${GITHUB_TOKEN}`。

项目级 MCP 在 `arc status` / `arc status --format json` 中会显示 `desired_scope` 与 `applied_scope`。对于只支持全局配置的 agent（例如 OpenClaw），默认会标记为 `skipped` / `requires_global_fallback`；显式传 `arc project apply --allow-global-fallback`，或在 `arc.toml` 的对应 `[[mcps]]` 下声明 `scope_fallback = "global"` 后，才会落到全局配置路径。

---

## subagent

管理全局 subagent 定义。裸调用 `arc subagent` 等同于 `arc subagent list`。

```bash
arc subagent list
arc subagent info reviewer

arc subagent install reviewer --prompt-file ./reviewer.md
arc subagent install reviewer --agent claude --agent codex --description "Code review helper" --prompt-file ./reviewer.md
arc subagent uninstall reviewer
```

- `arc subagent install` 只管理**全局**定义；项目级 subagent 请写入 `arc.toml` 的 `[[subagents]]`，再执行 `arc project apply`。
- `--prompt-file` 为必填；全局安装时读取命令行给出的文件内容并写入 agent 的全局 subagent 目录。
- `--agent` 可重复；不传时表示写入所有支持原生 subagent 的 agent。
- 项目级 `[[subagents]]` 的 `prompt_file` 相对路径以项目根目录为基准解析。
- 并非所有 agent 都支持 subagent。当前原生支持以 `arc status` 的 `subagent_supported` / `subagent` 字段为准。

---

## project apply

无 `arc.toml` 时：在**交互式**下先多选技能并写入 `arc.toml`，再安装项目级 skill、切换 provider，并应用项目级 MCP / subagent；**已有 `arc.toml`** 则直接执行这些步骤。从当前目录向上查找 `arc.toml`。

```bash
arc project apply
```

| 场景 | 行为 |
|------|------|
| **交互式、无 `arc.toml`** | 向导创建 `arc.toml`，再应用 |
| **非交互式、无 `arc.toml`、无 `--format json`**（通常为管道等纯文本路径） | 退出码 1 |
| **非交互式、无 `arc.toml`、`--format json`** | `stdout` 一条 `WriteResult`，`ok: false`，退出码 0 |
| **已有 `arc.toml`** | 解析并应用 |
| **有待装 skill、交互式、未传 `--agent` / `--all-agents`** | 多选目标 agent 后安装 |
| **有待装 skill、非交互式、未传 `--agent` / `--all-agents`** | 退出码 1，提示须指定目标 |

`--agent` 与 `--all-agents` 互斥。
命令行 `--agent` 与 `arc.toml` / 全局资源定义中的 `targets = [...]` 只能填写受支持的 agent id；拼写错误会直接报错，不会被当作“未检测到 agent”静默跳过。

**应用阶段步骤**：解析 `arc.toml`；**自动添加 `[[markets]]` 中声明且本地尚未配置的 market 源**；校验 provider 存在；若已激活则跳过切换；在文本模式下先列出 `require` 中每个 skill 的状态（**present (project)** / **will install** / **not in catalog**）；若未检测到任何支持项目级 skill 的 agent，则提前报错退出；若有缺失且可安装的 skill，则解析目标 agent 后安装；随后应用项目级 `[[mcps]]` 与 `[[subagents]]`，并对已从 `targets` 中移除的 agent 做 shrink 清理；警告不可用 skill；输出结果。

**`[[markets]]` 自动添加**：`arc.toml` 中声明的 market 源（`url` 必填）会在 `arc project apply` 时自动添加到本地配置，重复的 market（按 URL 生成的 id 判断）会自动跳过，不会报错。

**退出码与 JSON**：成功或仅跳过不可用 skill 时为 0；`arc.toml` 解析失败、provider 名无效，或非交互式下有待装 skill 却未传 `--agent` / `--all-agents` 时为 1。`--format json` 下，部分「未就绪」场景可能 **exit 0 但 `WriteResult.ok == false`**，须以 JSON 为准。

---

## project edit

交互编辑 `[skills] require`（保留 `[provider]` 与 `version`）。仅**交互式**。

```bash
arc project edit
```

| 场景 | 行为 |
|------|------|
| **交互式、有 `arc.toml`** | 多选写回；需同步再运行 `arc project apply` |
| **交互式、无 `arc.toml`** | 报错，提示先 `arc project apply` |
| **非交互式**（含 `--format json`） | `WriteResult` `ok: false`，不执行编辑 |

`project edit` 会同步 `[[markets]]` 与当前选中的 market skill：不再被任何已选 skill 引用的自动关联 market 会从 `arc.toml` 移除；无关的现有 market 保留。

---

## arc.toml 格式

```toml
# arc.toml — arc-kit project configuration
# Safe to commit. Contains no secrets.

version = 1

[provider]
name = "aicodemirror"

[[markets]]
url = "https://github.com/team/skills.git"

[[markets]]
url = "https://github.com/anthropics/skills.git"

[skills]
require = [
  "architecture-review",
  "db-migration",
]

[[mcps]]
name = "filesystem"
targets = ["claude", "codex"]
transport = "stdio"
command = "npx"
args = ["@modelcontextprotocol/server-filesystem", "."]

[[subagents]]
name = "reviewer"
targets = ["claude", "codex"]
prompt_file = "reviewer.md"
```

所有 section 可选，空文件合法。未知字段（如 `api_key`）会导致解析失败（退出码 1）。

### `version` — 配置版本

当前固定为 `1`；省略时也会按 `1` 解析。

### `[provider]` — 项目级 provider 对齐

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 否 | 要对齐的 provider profile 名称 |

只会作用于支持 provider 的 agent（当前为 Claude Code 与 Codex）。若同名 provider 在多个 agent 中都存在，`arc project apply` 会分别对命中的 agent 执行切换。

### `[skills]` — 项目级 skill 依赖

| 字段 | 必填 | 说明 |
|------|------|------|
| `require` | 否 | 需要在项目内落地的 skill 名称列表 |

### `[[markets]]` — 项目级 Market 源

项目可以声明其 skill 依赖所需的 market 源。`arc project apply` 会自动将缺失的 market 添加到本地配置。

| 字段 | 必填 | 说明 |
|------|------|------|
| `url` | 是 | Git 仓库地址（HTTPS 或 SSH） |

重复的 market（按 URL 自动生成的 id 判断）会自动跳过，不会重复添加。

### `[[mcps]]` — 项目级 MCP 定义

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | MCP 名称，需匹配 `^[a-z0-9][a-z0-9-_]{0,63}$` |
| `targets` | 否 | 目标 agent id 列表；省略时按支持该资源类型的 agent 解析 |
| `transport` | 是 | `stdio`、`sse` 或 `streamable_http` |
| `command` | `stdio` 必填 | 本地命令 |
| `args` | 否 | 命令参数列表 |
| `env` | 否 | 环境变量映射 |
| `url` | 远程 transport 必填 | SSE / Streamable HTTP 地址 |
| `headers` | 否 | 请求头映射 |
| `description` | 否 | 人类可读描述 |
| `scope_fallback` | 否 | 仅项目级 MCP 可用；当前只支持 `"global"` |

- `targets` 只能填写受支持的 agent id；拼写错误会直接报错。
- `stdio` 不能同时设置 `url`；`sse` / `streamable_http` 不能同时设置 `command`。
- `env` / `headers` 中涉及 secret 的键必须使用环境变量占位符，例如 `${API_KEY}` 或 `Bearer ${API_KEY}`。
- 对只支持全局 MCP 配置的 agent，可用 `scope_fallback = "global"` 单独声明 fallback；也可在命令行统一传 `--allow-global-fallback`。

示例：

```toml
[[mcps]]
name = "github"
targets = ["claude", "openclaw"]
transport = "streamable_http"
url = "https://api.github.com/mcp"
headers = { authorization = "Bearer ${GITHUB_TOKEN}" }
scope_fallback = "global"
```

### `[[subagents]]` — 项目级 subagent 定义

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | subagent 名称，需匹配 `^[a-z0-9][a-z0-9-_]{0,63}$` |
| `description` | 否 | 人类可读描述 |
| `targets` | 否 | 目标 agent id 列表；省略时按支持该资源类型的 agent 解析 |
| `prompt_file` | 是 | 提示词文件路径；相对路径相对于项目根目录 |

示例：

```toml
[[subagents]]
name = "reviewer"
targets = ["claude", "codex"]
description = "Repository reviewer"
prompt_file = "reviewer.md"
```

---

## completion

生成 shell 补全到 `~/.arc-cli/completions/`（bash、zsh、fish、powershell、elvish）。

```bash
arc completion zsh
# 按提示在 ~/.zshrc 中 source 生成的 arc.zsh
```

升级 arc 后重新运行以更新补全。
