# 交互与自动化设计

arc-kit 同时服务**人在终端操作**与**机器/脚本/Agent 集成**。只有两类运行语义：**交互式**与**非交互式**；不存在并列的第三种模式。非交互式里再区分纯文本和 `--format json`，二者都是自动化路径。

用户可见行为与命令细节见 [用户手册](../user/guide.md)。

## 交互式与非交互式

| 模式 | 条件 |
|------|------|
| 交互式 | 标准输入与标准输出均为终端，且未指定 `--format json` |
| 非交互式 | 无 TTY，或指定了 `--format json` |

`--format json` 优先于 TTY：即使在交互终端里使用 `--format json`，也走非交互式 JSON 输出，不弹 TUI。

## JSON 与退出码

读取类与多数写入类在 JSON 路径下输出稳定 JSON，顶层含 `schema_version`。当前 schema version 为 `"5"`。

`status` JSON 顶层模块为：

- `project`
- `agents`
- `catalog`
- `actions`

退出码约定：

| 场景 | 退出码 |
|------|--------|
| 成功 | 0 |
| 配置文件解析失败 | 1 |
| `status` 有缺失、部分落地或 unavailable skill | 0 |
| 非交互式缺必要参数 | 1 |
| `arc provider test` 有失败项 | 1 |
| JSON 序列化失败 | 1 |

写入类在「缺 `arc.toml`」等场景可能 `exit 0` 但 `WriteResult.ok == false`，自动化须以 JSON 的 `ok` / `message` 为准。

## 产品约束

### 只读命令必须支持 JSON

以查询、列举、汇总为主、不做破坏性写入的命令须实现 `--format json`：顶层 `schema_version`，字段稳定、无 ANSI。当前包括：

- `arc status`
- `arc market list`
- `arc skill list`
- `arc skill info <name>`
- `arc provider list`
- `arc provider test`
- `arc project edit` 的结构化失败结果

已登记例外：

- `arc version`
- `arc`（无子命令，仅 `--help`）
- `arc completion`

### 写入命令必须有非交互路径

交互式下若提供向导、多选、确认框，须同时提供显式参数，使非交互式在不读 stdin 的情况下完成同一语义。

当前写入命令的一键路径：

| 命令 | 一键路径 |
|------|----------|
| `skill install` / `uninstall` | 非交互式须提供名称等，否则报错 |
| `provider use` | 非交互式须提供名称，必要时提供 `--agent` |
| `market add` / `remove` / `update` | 参数齐全，可非交互 |
| `project apply` | 有 `arc.toml` 且需装项目 skill 时，非交互式须 `--agent` 或 `--all-agents` |
| `project edit` | 当前仅交互式编辑器；`--format json` 只返回失败结果 |

## 项目配置

`arc.toml` 支持：

- `version`
- `[provider]`
- `[skills]`
- `[[markets]]`

`[mcps]` 与 `[subagents]` 已移除，解析时会按未知字段拒绝。

`arc project apply` 在交互式且无 `arc.toml` 时进入单屏 `Project Skills` 编辑器创建配置；非交互式且无 `arc.toml` 时，纯文本路径报错并 exit 1，`--format json` 输出 `WriteResult.ok == false` 且 exit 0。

## TUI 边界

`dialoguer` 的交互调用与主题渲染仅存在于 `arc-tui`；`arc-core` 不依赖 UI 库。

列表型 TUI 在渲染时须按当前终端视口宽度裁剪每一行，不能依赖终端自动换行。

## 资源家族基线

当前完整资源家族只有 `skill`：

| Verb | 面向人的交互式语义 | 面向 Agent 的非交互式语义 |
|------|--------------------|---------------------------|
| `list` | TTY 下 browser，可 drill down 到详情 | 纯文本可 pipe；`--format json` 稳定输出集合 |
| `info` | 可从 `list` drill down；直调 `info <name>` 可查单项 | `info <name>` 明确查询单项；`--format json` 稳定输出详情 |
| `install` | 省略名称时进入向导 | 显式名称和目标 agent；缺参报错 |
| `uninstall` | 省略名称时从已安装项选择 | 显式名称和目标 agent 或 `--all`；缺参报错 |

新增资源家族时，按整个 `list / info / install / uninstall` 家族判断是否同时支持人和 Agent。

## 反例

- 只按 TTY 判断、忽略 `--format json`
- 在 `arc-core` 里 `println!`
- JSON 里混入 ANSI
- 非交互式仍调用 `dialoguer::Input::interact()`
