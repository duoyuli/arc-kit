# 交互与自动化设计 / Interactive & Non-Interactive Design

arc-kit 同时服务**人在终端操作**与**机器/脚本/Agent 集成**。只有两类运行语义：**交互式**与**非交互式**；不存在并列的第三种模式。非交互式里再区分纯文本和 `--format json`，二者都是自动化路径。

arc-kit serves both **human terminal use** and **machine/script/agent integration**. There are only two runtime semantics: **interactive** and **non-interactive**; there is no third mode. Within non-interactive, plain text and `--format json` are both automation paths.

用户可见行为与命令细节见 [用户手册](../user/guide.md)。

User-visible behavior and command details are in the [user manual](../user/guide.md).

## 交互式与非交互式 / Interactive vs Non-Interactive

| 模式 / Mode | 条件 / Condition |
|------|------|
| 交互式 / Interactive | 标准输入与标准输出均为终端，且未指定 `--format json` / Both stdin and stdout are TTYs, and `--format json` is not specified |
| 非交互式 / Non-Interactive | 无 TTY，或指定了 `--format json` / No TTY, or `--format json` is specified |

`--format json` 优先于 TTY：即使在交互终端里使用 `--format json`，也走非交互式 JSON 输出，不弹 TUI。

`--format json` takes precedence over TTY: even in an interactive terminal, `--format json` triggers non-interactive JSON output without launching TUI.

## JSON 与退出码 / JSON & Exit Codes

读取类与多数写入类在 JSON 路径下输出稳定 JSON，顶层含 `schema_version`。当前 schema version 为 `"5"`。

Read commands and most write commands output stable JSON under the JSON path, with a top-level `schema_version`. Current schema version is `"5"`.

`status` JSON 顶层模块为 / Top-level modules in `status` JSON:

- `project`
- `agents`
- `catalog`
- `actions`

退出码约定 / Exit code conventions:

| 场景 / Scenario | 退出码 / Exit Code |
|------|--------|
| 成功 / Success | 0 |
| 配置文件解析失败 / Config parse failure | 1 |
| `status` 有缺失、部分落地或 unavailable skill / `status` has missing, partial, or unavailable skills | 0 |
| 非交互式缺必要参数 / Non-interactive missing required params | 1 |
| `arc provider test` 有失败项 / `arc provider test` has failures | 1 |
| JSON 序列化失败 / JSON serialization failure | 1 |

写入类在「缺 `arc.toml`」等场景可能 `exit 0` 但 `WriteResult.ok == false`，自动化须以 JSON 的 `ok` / `message` 为准。

Write commands may `exit 0` with `WriteResult.ok == false` in cases like missing `arc.toml`; automation must check the JSON `ok` / `message` fields.

## 产品约束 / Product Constraints

### 只读命令必须支持 JSON / Read Commands Must Support JSON

以查询、列举、汇总为主、不做破坏性写入的命令须实现 `--format json`：顶层 `schema_version`，字段稳定、无 ANSI。当前包括：

Commands that query, list, or summarize without destructive writes must implement `--format json`: top-level `schema_version`, stable fields, no ANSI. Currently includes:

- `arc status`
- `arc market list`
- `arc skill list`
- `arc skill info <name>`
- `arc provider list`
- `arc provider test`
- `arc project edit` 的结构化失败结果 / `arc project edit` structured failure result

已登记例外 / Registered exceptions:

- `arc version`
- `arc`（无子命令，仅 `--help`）/ `arc` (no subcommand, only `--help`)
- `arc completion`

### 写入命令必须有非交互路径 / Write Commands Must Have Non-Interactive Paths

交互式下若提供向导、多选、确认框，须同时提供显式参数，使非交互式在不读 stdin 的情况下完成同一语义。

If interactive mode provides wizards, multi-select, or confirmations, explicit parameters must also be available so non-interactive mode can achieve the same result without reading stdin.

当前写入命令的一键路径 / Current one-shot paths for write commands:

| 命令 / Command | 一键路径 / One-shot Path |
|------|----------|
| `skill install` / `uninstall` | 非交互式须提供名称等，否则报错 / Non-interactive requires name; errors otherwise |
| `provider use` | 非交互式须提供名称，必要时提供 `--agent` / Non-interactive requires name, and `--agent` when needed |
| `market add` / `remove` / `update` | 参数齐全，可非交互 / Fully parameterizable, works non-interactively |
| `project apply` | 有 `arc.toml` 且需装项目 skill 时，非交互式须 `--agent` 或 `--all-agents` / With `arc.toml` requiring project skills, non-interactive needs `--agent` or `--all-agents` |
| `project edit` | 当前仅交互式编辑器；`--format json` 只返回失败结果 / Currently interactive-only editor; `--format json` returns failure only |

## 项目配置 / Project Configuration

`arc.toml` 支持 / Supports:

- `version`
- `[provider]`
- `[skills]`
- `[[markets]]`

`[mcps]` 与 `[subagents]` 已移除，解析时会按未知字段拒绝。

`[mcps]` and `[subagents]` have been removed and are rejected as unknown fields during parsing.

`arc project apply` 在交互式且无 `arc.toml` 时进入单屏 `Project Skills` 编辑器创建配置；非交互式且无 `arc.toml` 时，纯文本路径报错并 exit 1，`--format json` 输出 `WriteResult.ok == false` 且 exit 0。

`arc project apply` in interactive mode without `arc.toml` opens a single-screen `Project Skills` editor to create config; in non-interactive mode without `arc.toml`, plain text errors with exit 1, `--format json` outputs `WriteResult.ok == false` with exit 0.

## TUI 边界 / TUI Boundaries

`dialoguer` 的交互调用与主题渲染仅存在于 `arc-tui`；`arc-core` 不依赖 UI 库。

`dialoguer` interactions and theme rendering exist only in `arc-tui`; `arc-core` has no UI library dependency.

列表型 TUI 在渲染时须按当前终端视口宽度裁剪每一行，不能依赖终端自动换行。

List-type TUIs must clip each line to the current terminal viewport width at render time; do not rely on terminal auto-wrapping.

## 资源家族基线 / Resource Family Baseline

当前完整资源家族只有 `skill`：

The only complete resource family today is `skill`:

| Verb | 面向人的交互式语义 / Human Interactive Semantics | 面向 Agent 的非交互式语义 / Agent Non-Interactive Semantics |
|------|--------------------|---------------------------|
| `list` | TTY 下 browser，可 drill down 到详情 / TTY browser with drill-down to details | 纯文本可 pipe；`--format json` 稳定输出集合 / Plain text pipeable; `--format json` stable collection |
| `info` | 可从 `list` drill down；直调 `info <name>` 可查单项 / Drill down from `list`; `info <name>` queries single item | `info <name>` 明确查询单项；`--format json` 稳定输出详情 / `info <name>` queries single item; `--format json` stable detail |
| `install` | 省略名称时进入向导 / Omitting name launches wizard | 显式名称和目标 agent；缺参报错 / Explicit name and target agent; errors on missing params |
| `uninstall` | 省略名称时从已安装项选择 / Omitting name selects from installed items | 显式名称和目标 agent 或 `--all`；缺参报错 / Explicit name and target agent or `--all`; errors on missing params |

新增资源家族时，按整个 `list / info / install / uninstall` 家族判断是否同时支持人和 Agent。

When adding a new resource family, evaluate the full `list / info / install / uninstall` family for both human and agent support.

## 反例 / Anti-Patterns

- 只按 TTY 判断、忽略 `--format json` / Judging only by TTY, ignoring `--format json`
- 在 `arc-core` 里 `println!` / Using `println!` in `arc-core`
- JSON 里混入 ANSI / Mixing ANSI into JSON
- 非交互式仍调用 `dialoguer::Input::interact()` / Calling `dialoguer::Input::interact()` in non-interactive mode
