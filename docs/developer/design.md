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

**例外**：`arc project apply` 在**交互式**且**无 `arc.toml`** 时先进入共享的单屏 `Project Requirements` 编辑器创建配置；**非交互式**且无 `arc.toml` 时，纯文本路径报错并 **exit 1**，`--format json` 时输出 `WriteResult.ok == false` 且 **exit 0**；**已有 `arc.toml`** 时 `apply --format json` 按写入类正常输出。`arc project edit` 不提供一键非交互编辑路径，`--format json` 只返回结构化失败信息，不会打开编辑器。

### 两项硬约束（产品标准）

1. **只读类命令必须支持 JSON**  
   以查询、列举、汇总为主、不做破坏性写入的命令（如 `status`、`skill list`、`skill info`、`mcp list`、`mcp info`、`subagent list`、`subagent info`、`provider list`、`market list`、`provider test`）须实现 `--format json`：顶层 `schema_version`，字段稳定、无 ANSI。无法提供时须在 [用户手册](../user/guide.md) 或本文登记例外。`arc version` 为**已登记例外**（初始化状态目录前即退出）。

2. **交互型写入命令必须提供「一键」非交互路径**  
   **交互式**下若提供向导、多选、确认框，须同时提供**显式参数**，使**非交互式**（含 CI、管道、`--format json`）在**不读 stdin** 下完成同一语义。缺参报错应提示所需 flag。

**Agent 与项目级 skill**：部分 agent（当前为 **OpenClaw**）在实现上**不提供**仓库内项目级 skill 目录；`arc project apply` 仅对 `CodingAgentSpec.supports_project_skills == true` 的已检测 agent 安装与校验。详见 [用户手册](../user/guide.md) 中的表格。

---

## 实现机制摘要

### TTY 检测

```rust
let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
```

用于：`skill list`（是否 TUI）、`skill install/uninstall`（无名称时是否向导）、`provider use`（无名称时是否进入 tab 选择器）、`mcp list/info/install/uninstall`（是否进入 browse / pick / wizard）、`subagent install/uninstall`（无名称时是否进入向导）、`project apply`（无 `arc.toml` 是否进入共享编辑器；有待装 skill 时是否多选目标 agent）、`project edit`（交互式编辑器与 JSON 失败分支）。

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

**已知缺口**：`arc project edit` 当前没有一键非交互编辑参数；临时做法是直接编辑 `arc.toml` 后再执行 `arc project apply`。`--format json` 只返回失败结果，不会完成编辑。若增加 `--require` 等，应更新本文与 [用户手册](../user/guide.md)。

**当前交互定义**：`arc project apply`（首次创建）与 `arc project edit` 复用同一个单屏编辑器。默认进入 `All` tab，可全局搜索 `skill` / `mcp` / `subagent`；`tab` / `backtab` 或 `←→` 切换分类，`space` 勾选，`enter` 保存，`esc` 取消且不写文件。已有但不在当前 catalog 中的名称要以 `missing from catalog` 显示，不能无声丢失。

### 规则 3：JSON schema 与版本

顶层含 `schema_version`（当前为 `"5"`）。破坏性变更升版本；兼容新增字段可不升。可选字段避免输出无意义 `null`。消费方应校验 `schema_version`。`status` 在 v5 固定拆为 `project`、`agents`、`catalog`、`mcps`、`subagents`、`actions` 六个模块；其中 `project.summary` 暴露 `ready / partial / missing / unavailable` 聚合，`mcps` / `subagents` 暴露 capability rollout 与 drift 状态，`actions` 暴露建议动作。

### 规则 4：退出码

新命令须先定义退出码语义并写入文档。Agent 以退出码判成败，以 JSON 取结构化数据，**不要**依赖解析人类可读文本。

### 规则 5：TUI 仅存在于 `arc-tui`

`dialoguer` 的交互调用与主题渲染仅存在于 `arc-tui`；`arc-cli` 分支调用；`arc-core` 不依赖 UI 库，但可为 TUI 准备中性的数据映射。
文本列表布局、TTY / JSON 模式判定等 CLI 细节同样应留在 `arc-cli` / `arc-tui`，不要为了复用把终端排版 helper 放回 `arc-core`。

列表型 TUI 在渲染时须按当前终端视口宽度裁剪每一行，不能依赖终端自动换行；否则在窄窗口下重绘与清屏会出现残影或重复内容。

### 规则 6：资源类命令按统一 verb 契约设计

适用于同一资源家族同时暴露 `list` / `info` / `install` / `uninstall` 四个动词的命令；当前主要是 `skill`、`mcp`、`subagent`。

**强约束**：对这类资源，是否“支持 for 人”必须按**整个家族**判断，不能因为 `install` 有向导，就宣称整组命令已经同时支持人和 Agent。

**推荐基线**：后续新增或补齐资源家族时，优先按 `skill` 当前模式实现：

- `list` 在 TTY 下提供 browser / fuzzy browse，并可直接 drill down 到详情
- `info` 保留显式 `<name>` 的 Agent 路径；若 `list` 已能 drill down，则 `info` 不必强制再做第二套交互选择
- `install` / `uninstall` 采用 `name: Option<String>`，TTY 下省略名称进入 wizard，非交互下缺参立即报错
- 所有只读入口都必须提供 `--format json`
- 共享的 TTY / JSON / 缺参判定优先收口到公共 helper，避免每个资源命令各写一套分支

| Verb | 面向人的交互式语义 | 面向 Agent 的非交互式语义 |
|------|--------------------|---------------------------|
| `list` | 资源浏览入口。交互式下应能浏览、筛选，且最好能直接进入详情或下一步动作 | 纯文本可 pipe；`--format json` 必须稳定输出集合 |
| `info` | 人不应被迫先记住精确名称。必须满足二选一：① `list` 可 drill down 到详情；② `info` 省略 `<name>` 时可交互选择 | `info <name>` 明确查询单项；`--format json` 输出稳定详情 |
| `install` | 省略 `<name>` 时进入选择器/向导；若还需目标 agent、prompt file 等，交互式逐步补齐 | 显式参数一次完成同一语义；不得读 stdin |
| `uninstall` | 省略 `<name>` 时从“已安装项”里选择；必要时继续选目标 agent/确认 | 显式参数一次完成同一语义；不得读 stdin |

**等价性要求**：交互式路径与参数路径必须落到**同一领域操作**，只是输入方式不同，不能做成两套语义不同的功能。

### 规则 7：识别“是否已经支持 for 人”要看入口完备性

评审 `skill` / `mcp` / `subagent` 这类资源家族时，按下面顺序判断：

1. **先看家族是否完整**  
   该资源是否同时定义了 `list` / `info` / `install` / `uninstall` 四个入口？如果是，就必须按家族契约审视四个 verb。
2. **再看人类是否能在不知道名字时完成任务**  
   `list` 是否提供浏览入口？`info` 是否能通过 `list` drill down，或省略名称进入选择？`install` / `uninstall` 是否能在 TTY 下通过选择器完成？
3. **最后看 Agent 是否能一键完成同一语义**  
   非交互式是否完全不读 stdin？缺参是否明确报错？`--format json` 是否稳定、无 ANSI？

**判定标准**：

- 四个 verb 都满足家族契约：才算“同时支持 for 人和 for Agent”
- 只有 `install` 有向导：只算“部分支持 for 人”
- 只有纯文本列表、必须记名字再 `info` / `uninstall`：不算“for 人完备”

### 规则 8：代码评审时的识别信号必须可见

除了产品语义，还要能从代码里快速识别实现是否达标。

| 关注点 | 应看到的实现信号 | 反例 |
|--------|------------------|------|
| `list` 的人类入口 | `is_terminal()` 分支进入 `arc_tui` 的 browse/select helper | 全部分支都只是 `println!` 列表 |
| `info` 的人类入口 | 要么 `list` 的 browser callback 直接渲染详情；要么 `info` 的 `name` 为 `Option<String>` 且在 TTY 下走选择器 | `info` 只有必填 `String name`，同时 `list` 也不能 drill down |
| `install` 的人类入口 | `name: Option<String>`，`None` + TTY 时进入 wizard；附加必填项逐步补齐 | `install` 必须传 `<name>`，或在非交互式读 stdin |
| `uninstall` 的人类入口 | `name: Option<String>`，`None` + TTY 时从已安装集选择 | `uninstall` 必须先手敲名称且没有浏览/选择入口 |
| Agent 路径 | `!is_tty || --format json` 分支下显式缺参报错；JSON 分支稳定输出 | 非交互式仍然进入 `dialoguer` 或依赖人类文本解析 |

这些信号是**必要条件，不是充分条件**：例如 `name: Option<String>` 只是表明“可能支持交互入口”，还要确认它在 TTY 下真的进入选择器，而不是继续报错。

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
| `arc project edit --format json` | `WriteResult`，`ok == false` |
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
| `arc mcp info [name]` | `McpInfoOutput`（存在时；TTY 下可省略名称进入选择） |
| `arc subagent info <name>` | `SubagentInfoOutput`（存在时） |
| `arc provider` / `provider list` | `ProviderListOutput` |
| `arc provider test` | `ProviderTestOutput` |
| `arc project edit` | `WriteResult`（仅错误信息，不执行编辑） |

### 写入命令与非交互路径

| 命令 | 一键路径 |
|------|----------|
| `skill install` / `uninstall` | **非交互式**须提供名称等，否则报错 |
| `mcp install` / `uninstall` | **非交互式**须提供名称等，否则报错；自定义 install/define 的额外字段也须全部显式给出 |
| `subagent install` / `uninstall` | **非交互式**须提供名称；若未命中 catalog，则还须显式 `--prompt-file` |
| `provider use` | **非交互式**须提供名称（及必要时 `--agent`） |
| `market add` / `remove` / `update` | 参数齐全，可非交互；`market update` 的 `--format json` 在 **全局 skill 维护**（清理 + 按 registry 重装）任一步失败时为 `WriteResult.ok == false`（`items` 含失败项），进程仍 **exit 0** |
| `project apply` | 有 `arc.toml` 且需装项目 skill 时，**非交互式**须 `--agent` 或 `--all-agents`；**无 `arc.toml` 且首次创建**仅**交互式**单屏编辑器，`--format json` 只返回 `WriteResult.ok == false`；自动化若要创建仍需手写或拷贝 `arc.toml` |
| `project edit` | 当前仅交互式单屏编辑器；`--format json` 只返回 `WriteResult.ok == false`，不执行编辑；替代方案是编辑 `arc.toml` 后 `arc project apply` |

### 资源家族（`skill` / `mcp` / `subagent`）统一验收

下表按“统一 verb 契约”验证**当前实现**，用于识别哪些资源家族已经能宣称“同时支持 for 人和 for Agent”。

| 资源家族 | `list` | `info` | `install` | `uninstall` | 结论 |
|----------|--------|--------|-----------|-------------|------|
| `skill` | **通过**：TTY 下 browser，非交互有 text/json | **通过**：可从 `list` drill down；直调 `info <name>` 也支持 JSON | **通过**：TTY 下省略名称进 wizard；非交互显式参数 | **通过**：TTY 下省略名称进 wizard；非交互显式参数 | 当前基线，可作为规范参考实现 |
| `mcp` | **通过**：TTY 下 browser，非交互有 text/json | **通过**：TTY 下可省略名称进入选择；直调 `info <name>` 也支持 JSON | **通过**：预设安装支持 TTY 选择；非交互参数路径完整；自定义安装仍以参数路径为主 | **通过**：TTY 下省略名称可从已安装项选择；非交互显式参数 | 当前已满足统一标准，可作为第二个参考实现 |
| `subagent` | **通过**：TTY 下 browser，非交互有 text/json | **通过**：TTY 下可省略名称进入选择；直调 `info <name>` 也支持 JSON | **通过**：TTY 下省略名称进 wizard；非交互显式参数 | **通过**：TTY 下省略名称可从已安装项选择；非交互显式参数 | 当前已满足统一标准 |

### 改进优先级（工程）

1. **P1**：为 `project edit` 增加显式参数，或产品层确认长期仅编辑器 + `apply`。
2. **P2**：`project apply` 在无 `arc.toml` 时可选非交互创建参数。
3. **P3**：为 `project apply` 在无 `arc.toml` 时设计显式非交互初始化参数（若产品决定支持）。

### 维护说明

发版前可结合 [开发与贡献](development.md) 中的回归门禁，对 `--format json` 与非交互式路径抽样验证。
