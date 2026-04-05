# 交互与自动化设计

arc-kit 同时服务**人在终端操作**与**机器/脚本/Agent 集成**。只有两类运行语义：**交互式**与**非交互式**；不存在并列的「第三种模式」。非交互式里再区分输出是**纯文本**还是 **`--format json`**，二者都是自动化路径，不是与人机交互并列的另一套「模式」。

用户可见行为与命令细节见 [用户手册](../user/guide.md)。本文说明判定方式、JSON 约定、新功能规则，以及**当前实现与标准的对照**。

---

## 交互式与非交互式

### 判定（与 `arc-cli` 实现一致）

| 模式 | 条件 |
|------|------|
| **交互式** | 标准输入**与**标准输出均为终端（TTY），且**未**指定 `--format json`（即 `--format text` 或默认） |
| **非交互式** | 任一不满足上表：无 TTY（管道/重定向/CI），**或**指定了 `--format json` |

**`--format json` 优先于 TTY**：即使在交互终端里使用 `--format json`，也走非交互式 JSON 输出，不弹 TUI、不按交互式渲染。

### 非交互式下的输出

| 输出 | 条件 | 用途 |
|------|------|------|
| **纯文本** | 非交互式且 `--format` 为 `text`（默认） | `grep`、pipe、日志 |
| **JSON** | `--format json`（与是否 TTY 无关） | 稳定结构、`schema_version`、语义退出码 |

读取类与多数写入类在 JSON 路径下输出稳定 JSON。

**例外**：`arc project apply` 仅在**交互式**且**无 `arc.toml`** 时先走创建向导（该段不按 JSON 输出）；**非交互式**且无 `arc.toml` 时：纯文本路径报错并 **exit 1**，`--format json` 时输出 `WriteResult.ok == false` 并 **exit 0**（见用户手册）；**已有 `arc.toml`** 时 `apply --format json` 按写入类正常输出。

### 两项硬约束（产品标准）

1. **只读类命令必须支持 JSON**  
   以查询、列举、汇总为主、不做破坏性写入的命令（如 `status`、`skill list`、`skill info`、`provider list`、`market list`、`provider test`）须实现 `--format json`：顶层 `schema_version`，字段稳定、无 ANSI。无法提供时须在 [用户手册](../user/guide.md) 或本文登记例外。`arc version` 为**已登记例外**（初始化状态目录前即退出）。

2. **交互型写入命令必须提供「一键」非交互路径**  
   **交互式**下若提供向导、多选、确认框，须同时提供**显式参数**，使**非交互式**（含 CI、管道、`--format json`）在**不读 stdin** 下完成同一语义。缺参报错应提示所需 flag。

**Agent 与项目级 skill**：部分 agent（当前为 **OpenClaw**）在实现上**不提供**仓库内项目级 skill 目录；`arc project apply` 仅对 `CodingAgentSpec.supports_project_skills == true` 的已检测 agent 安装与校验。详见 [用户手册](../user/guide.md) 中的表格。

---

## 实现机制摘要

### TTY 检测

```rust
let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
```

用于：`skill list`（是否 TUI）、`skill install/uninstall`（无名称时是否向导）、`project apply`（无 `arc.toml` 是否创建；有待装 skill 时是否多选目标 agent）、`project edit`（仅交互式）。

写入类在**非交互式**且缺参时须报错退出，不得阻塞于 stdin。

### `--format json` 分支

```rust
if *fmt == OutputFormat::Json { ... }
```

`--format` 为全局 flag；各子命令 JSON 语义以 [用户手册](../user/guide.md) 为准。

### 退出码约定

| 场景 | 退出码 |
|------|--------|
| 成功 | 0 |
| 配置文件解析失败 | 1 |
| `status` 有缺失、部分落地或 unavailable skill（纯文本或 `--format json`） | 0（见输出或 JSON `project.summary` / `project.skills`） |
| 非交互式缺必要参数 | 1 |
| `arc provider test` 有失败项 | 1 |
| JSON 序列化失败 | 1 |

**与 JSON 的关系**：写入类在「缺 `arc.toml`」等场景可能 **`exit 0` 但 `WriteResult.ok == false`**，自动化须以 JSON 的 `ok` / `message` 为准。

---

## 新功能必须遵守的规则

### 规则 1：只读命令——交互式 + 非交互式（含强制 JSON）

须覆盖交互式；非交互式须含纯文本与 **`--format json`**（**对只读命令为强制**，登记例外除外）。

**禁止**：只实现交互式；非交互式下崩溃或乱码；只读缺少 JSON；JSON 混入 ANSI。

### 规则 2：写入命令——交互式与一键非交互双路径

适用于 `skill install`、`provider use`、`project apply` 等。

```
交互式    →  允许向导、多选（便捷路径）
非交互式  →  须能通过显式参数一键完成，不得读 stdin（含 `--format json`）
```

**设计检查表**：交互式每一步对应的 CLI 参数？非交互式最小参数集？

**已知缺口**：`arc project edit` 当前仅**交互式**；临时做法为编辑 `arc.toml` 后 `arc project apply`；若增加 `--require` 等，应更新本文与 [用户手册](../user/guide.md)。

### 规则 3：JSON schema 与版本

顶层含 `schema_version`（当前为 `"3"`）。破坏性变更升版本；兼容新增字段可不升。可选字段避免输出无意义 `null`。消费方应校验 `schema_version`。`status` 在 v3 固定拆为 `project`、`agents`、`catalog` 三个模块；其中 `project.summary` 暴露 `ready / partial / missing / unavailable` 聚合，`project.skills` 暴露每个 required skill 的 rollout 细节。

### 规则 4：退出码

新命令须先定义退出码语义并写入文档。Agent 以退出码判成败，以 JSON 取结构化数据，**不要**依赖解析人类可读文本。

### 规则 5：TUI 仅存在于 `arc-tui`

`dialoguer` 的交互调用与主题渲染仅存在于 `arc-tui`；`arc-cli` 分支调用；`arc-core` 不依赖 UI 库，但可为 TUI 准备中性的数据映射。

列表型 TUI 在渲染时须按当前终端视口宽度裁剪每一行，不能依赖终端自动换行；否则在窄窗口下重绘与清屏会出现残影或重复内容。

---

## 新命令设计清单

1. **读还是写？** 读 → 交互式 + 非交互式 JSON（除例外）；写 → 定义显式参数的一键路径。
2. **JSON 长什么样？** 在 `arc-cli/src/format.rs` 定义，`snake_case`，含 `schema_version`。
3. **退出码语义？** 文档化。
4. **TUI 是否必要？** 仅在为交互用户带来明确价值时加入。

---

## 反例（常见错误）

- 只按 TTY 判断、忽略 `--format json`，导致自动化仍收到纯文本。
- 在 `arc-core` 里 `println!` 给用户。
- JSON 里混入 ANSI。
- **非交互式**仍 `dialoguer::Input::interact()` 阻塞。

---

## 快速判断表

| 场景 | 正确行为 |
|------|----------|
| `arc skill list`（交互式） | TUI browser |
| `arc skill list`（管道） | 纯文本列表（每 skill 1–2 行，含 origin 与安装状态） |
| `arc skill list --format json` | JSON + `schema_version` |
| `arc skill install`（交互式，无名称） | 向导 |
| `arc skill install`（管道，无名称） | 退出 1 |
| `arc skill install foo`（管道） | 直接安装 |
| `arc provider use`（管道，无名称） | 退出 1 |
| `arc project apply --format json` | `WriteResult` |
| 失败 | 非 0，错误到 stderr |
| 成功 | 0，结果到 stdout |

---

## 实现对照与缺口

以下对照 **当前 `arc-cli` 实现** 与上文两项硬约束。行为变更后须同步更新本节与 [用户手册](../user/guide.md)。

### 只读命令与 `--format json`

**已登记例外（非「数据 JSON」）**

| 入口 | 说明 |
|------|------|
| `arc version` | 不解析 `--format`，见 [用户手册](../user/guide.md) |
| `arc`（无子命令） | 仅 `--help` |
| `arc completion` | 写补全脚本，非状态 API |

**数据类只读命令（成功路径）**

| 命令 | JSON |
|------|------|
| `arc status` | `StatusOutput`（项目解析失败时仍可输出，`project.state` 可能为 `invalid`） |
| `arc market list` | `MarketListOutput` |
| `arc skill list` | `SkillListOutput` |
| `arc skill info <name>` | `SkillInfoOutput`（存在时） |
| `arc provider` / `provider list` | `ProviderListOutput` |
| `arc provider test` | `ProviderTestOutput` |

**偏差**：`arc skill info <name> --format json` 在 skill **不存在**时直接 `Err`，**stdout 无 JSON**。建议后续改为结构化错误 JSON，或长期登记为已知偏差。

### 写入命令与非交互路径

| 命令 | 一键路径 |
|------|----------|
| `skill install` / `uninstall` | **非交互式**须提供名称等，否则报错 |
| `provider use` | **非交互式**须提供名称（及必要时 `--agent`） |
| `market add` / `remove` / `update` | 参数齐全，可非交互；`market update` 的 `--format json` 在 **全局 skill 维护**（清理 + 按 registry 重装）任一步失败时为 `WriteResult.ok == false`（`items` 含失败项），进程仍 **exit 0** |
| `project apply` | 有 `arc.toml` 且需装项目 skill 时，**非交互式**须 `--agent` 或 `--all-agents`；**无 `arc.toml` 且首次创建**仅**交互式**向导，**无**等价 CLI 参数批量创建（自动化需手写或拷贝 `arc.toml`）→ **缺口** |
| `project edit` | 仅**交互式**，无显式非交互 flag → **缺口**；替代为编辑 `arc.toml` + `arc project apply` |

### 改进优先级（工程）

1. **P0**：为 `project edit` 增加显式参数，或产品层确认长期仅编辑器 + `apply`。
2. **P1**：`project apply` 在无 `arc.toml` 时可选非交互创建参数。
3. **P2**：`skill info` 在 not found 且 `--format json` 时输出结构化错误。

### 维护说明

发版前可结合 [开发与贡献](development.md) 中的回归门禁，对 `--format json` 与非交互式路径抽样验证。
