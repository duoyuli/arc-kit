# 状态机详情

本文档是状态机和 STATE.yaml schema 的**唯一权威来源**。其他文件应引用本文档而非重复内容。

## 状态机图

```
assigned → received → in-progress ⇌ blocked
                             ↑ ↓
                          submitted
                          ↙       ↘
                        done    rejected → in-progress（重做）

任何非终态 ──(leader 取消)──→ cancelled
blocked ──(leader 确认失败)──→ failed

终态：done / cancelled / failed
```

## 转换表

| 源状态 | 目标状态 | 触发方 | 说明 |
|--------|----------|--------|------|
| — | `assigned` | leader 手动 | 新建任务时设置初始状态 |
| `assigned` | `received` | Cron 自动 | 日志出现 received |
| `received` | `in-progress` | Cron 自动 | 日志出现 in-progress |
| `in-progress` | `blocked` | Cron 自动 | 日志出现 blocked |
| `blocked` | `in-progress` | Cron 自动 | 阻塞解除，日志出现 in-progress |
| `in-progress` | `submitted` | Cron 自动 | 日志出现 submitted |
| `submitted` | `done` | leader 手动 | 验收通过 |
| `submitted` | `rejected` | leader 手动 | 验收不通过，必须填 rejection_note |
| `rejected` | `in-progress` | Cron 自动 | follower 重做后写 in-progress 日志，Cron 自动覆盖 rejected |
| `assigned` / `received` / `in-progress` / `blocked` | `cancelled` | leader 手动 | 取消或改派，任何非终态均可 |
| `blocked` | `failed` | leader 手动 | follower 通过 blocked 日志申请，leader 评估后确认 |

## 状态定义

| 状态 | 谁设 | 含义 |
|---|---|---|
| `assigned` | leader agent 手动 | 已分派，等 follower 接单 |
| `received` | **cron 自动** | 日志出现 received 条目 |
| `in-progress` | **cron 自动** | 日志出现 in-progress 条目 |
| `blocked` | **cron 自动** | 日志出现 blocked 条目，等外部决策 |
| `submitted` | **cron 自动** | 日志出现 submitted 条目，待 leader agent 验收 |
| `rejected` | leader agent 手动 | 验收不通过，退回重做，必须填 `rejection_note` |
| `done` | leader agent 手动 | 验收通过 |
| `cancelled` | leader agent 手动 | 取消或改派 |
| `failed` | leader agent 手动 | follower 通过 blocked 日志申请，leader agent 评估后确认 |

## 关键规则

- `tasks-log.md` 是事件流（follower 写）；`STATE.yaml` 是当前状态真相（cron + leader agent 写）。
- follower 可写入日志的状态：`received` / `in-progress` / `blocked` / `submitted`，**不可自行写入任何终态**。
- `blocked ⇌ in-progress`：外部依赖解除后，follower 写 in-progress 日志，cron 自动回写 STATE。
- `rejected` 后的流程：leader agent 填 `rejection_note` → follower 读取原因 → 在新版本子目录重做 → 继续走 in-progress → submitted 流程。`rejected` 不是终态，Cron 1 在收到 follower 新的 in-progress 日志后会自动将状态从 `rejected` 覆盖为 `in-progress`，leader 无需手动清除。

## 状态分两类，写入方不同

| 类型 | 状态 | 写入方 | 说明 |
|---|---|---|---|
| 事实性（follower 行为可推断） | `received` / `in-progress` / `blocked` / `submitted` | **Cron 1 自动回写** | 从最新日志条目提取 |
| 判断性（需要决策） | `done` / `rejected` / `cancelled` / `failed` | **leader 手动写** | 验收 / 退回 / 取消 / 确认失败时 |

## STATE.yaml Schema

```yaml
slug: example-task
broadcast:
  mode: announce
  channel: feishu
  to: oc_xxxxx
accept_sla_min: 5
patrol_notes: ""
automation:
  patrol_sync_job_id: "11111111-1111-1111-1111-111111111111"
  patrol_trigger_job_id: "22222222-2222-2222-2222-222222222222"
tasks:
  - id: R1
    agent: two
    goal: 一句话描述
    deadline: 2026-03-13T23:00:00+08:00
    status: assigned
    depends_on: []
    rejection_note: ""
    artifacts: ""
    submitted_at: ""
    last_log_at: ""
```

## 字段说明

- `slug`：任务唯一标识符，用于目录名和群同步消息前缀。

- `broadcast`：**任务级唯一正式广播目标**。凡是该任务的进展 / 异常 / 完成同步，都应通过机器人按 `channel + to` 发到 IM 群，而不是发到"当前正在聊天的 OpenClaw 会话"。
  - 推荐字段：
    - `mode: announce`
    - `channel: feishu | telegram`
    - `to: <IM 群标识>`
  - Feishu 群示例：`channel: feishu` + `to: oc_xxx`
  - Telegram 群示例：`channel: telegram` + `to: -1001234567890`
  - **硬规则：正式广播看的是 IM 群路由，不是 OpenClaw session key。**
  - `broadcast_session` 只属于旧口径兼容字段，不再作为正式广播出口。

- `accept_sla_min`：任务进入 `submitted` 后，leader agent 验收回写的 SLA（分钟）。

- `patrol_notes`：巡检摘要。只写结论，不写长解释。格式为分号分隔的 `<taskID>: <结论>` 对，例如 `"R1: 日志停更; R2: blocked 等待决策"`。无异常时清空为 `""`。

- `automation.patrol_sync_job_id` / `automation.patrol_trigger_job_id`：双 cron 的唯一 jobId。**两者都回写成功，才算正式开工。**

- `tasks[].id`：任务 ID，如 `R1`、`R2`。

- `tasks[].agent`：执行此任务的 follower agent 名称。

- `tasks[].goal`：一句话描述任务目标。

- `tasks[].deadline`：ISO 8601 含时区 offset 的截止时间。

- `tasks[].status`：当前状态，见状态定义表。

- `tasks[].depends_on`：依赖的 task ID 列表。leader agent 在所有依赖 `done` 之前**不应分派**该任务。

- `tasks[].rejection_note`：leader agent 退回时说明原因，follower 重做时必读。

- `tasks[].artifacts`：当前任务最新提交的产物路径。

- `tasks[].submitted_at`：最近一次进入 `submitted` 的时间，由巡检或 leader 维护。

- `tasks[].last_log_at`：最近一条该任务日志时间，用于判断是否停更。
