# 交互与自动化设计

[English](design.md)

本文定义面向人、脚本和 coding agent 的命令语义。用户工作流见 [用户指南](../user/guide.zh-CN.md)；实现约定见 [development.zh-CN.md](development.zh-CN.md)。

## 运行模式

`arc-kit` 只有两种运行模式：

| 模式 | 条件 |
| --- | --- |
| 交互式 | stdin 和 stdout 都是 TTY，且没有指定 `--format json` |
| 非交互式 | 没有 TTY，或指定了 `--format json` |

`--format json` 优先于 TTY 检测。即使命令在终端中运行，只要使用 `--format json`，就必须走自动化路径，不得启动 TUI 或 `dialoguer` 流程。

## JSON 与退出码

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

## JSON 覆盖

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

## 写入命令

如果交互式命令提供向导、多选、确认或编辑器，非交互式路径必须由显式参数完成，并且不得读取 stdin。

当前一键路径：

| 命令 | 非交互式路径 |
| --- | --- |
| `skill install` / `skill uninstall` | 显式 name，并按需提供目标 agent 或 `--all` |
| `provider use` | 显式 provider name；存在歧义时提供 `--agent` |
| `market add` / `market remove` / `market update` | 由命令参数完整表达 |
| `project apply` | 项目 skill 需要安装时提供 `--agent` 或 `--all-agents` |
| `project edit` | 仅交互式编辑器；JSON 路径返回结构化失败，不打开编辑器 |

## 项目配置

`arc.toml` 支持：

- `version`
- `[provider]`
- `[[markets]]`
- `[skills]`

`[mcps]` 和 `[subagents]` 已移除，出现时会按未知字段拒绝。

当 `arc project apply` 在交互模式下运行且缺少 `arc.toml` 时，会打开项目 skill 编辑器创建配置。在非交互模式下缺少 `arc.toml` 时，纯文本路径以 `1` 退出；JSON 路径返回 `WriteResult.ok == false` 且退出码为 `0`。

## UI 边界

- 业务逻辑属于 `arc-core`。
- CLI 命令定义和用户输出属于 `arc-cli`。
- TUI 和 `dialoguer` 交互只属于 `arc-tui`。
- `arc-core` 不得向 stdout 输出，也不得依赖 UI 库。

列表型 TUI 必须按当前终端宽度裁剪每一行，不能依赖终端自动换行。

## 资源家族基线

当前唯一完整资源家族是 `skill`：

| 动词 | 交互式行为 | 非交互式行为 |
| --- | --- | --- |
| `list` | TTY browser，可 drill down 到详情 | 可 pipe 的文本和稳定 JSON 集合 |
| `info` | 从 list 进入详情，或直接查询 | 显式单项查询和稳定 JSON 详情 |
| `install` | 省略 name 时进入向导 | 显式 name 和目标 agent |
| `uninstall` | 省略 name 时从已安装项选择 | 显式 name 和目标 agent 或 `--all` |

新增其他资源家族时，需要同时评估 `list / info / install / uninstall` 是否支持人和 agent。

## 反例

- 只判断 TTY，忽略 `--format json`；
- JSON 中混入 ANSI；
- 在非交互模式调用 `dialoguer::Input::interact()`；
- 把文件系统或领域行为放进 `arc-cli`，而不是 `arc-core`；
- 新增只读命令但不提供 JSON 输出。
