# Cron 自动化系统

## 双 Cron 机制

两个 cron 职责分离，Cron 1：3 分钟运行，Cron 2 ：5 分钟运行，任务开始时由 leader agent 各创建一次。

**硬门槛：双 cron 没成功创建，不算开工。**
- 顺序必须固定：先建 `patrol-sync-<slug>`，再建 `patrol-trigger-<slug>`，拿到两个 jobId，回写 `STATE.yaml.automation`，然后才能发 `【开工】`。
- 任一 cron 创建失败时，**禁止**对群发送"【开工】<slug>"或默认任务已进入受控推进状态。
- 这时只能发"未开工/受阻"同步，说明原因（例如 gateway / cron / 权限异常），并立即重试、排障或转为手动高频同步。
- 若未起双 cron 就已分派任务，leader agent 仍需把该任务视为**未正式开工**，优先补齐 cron，再继续推进。

## Cron 1 — 状态同步

**职责**：isolated，便宜模型干脏活；只回写，不负责对群广播

```bash
openclaw cron add \
  --name "patrol-sync-<slug>" \
  --cron "*/3 * * * *" \
  --tz "Asia/Shanghai" \
  --session isolated \
  --model "openai-codex/gpt-5.3-codex" \
  --message "任务目录：~/.openclaw/tasks/<slug>/。读取 STATE.yaml + tasks-log.md，执行：1）自动同步事实性状态；2）回写 artifacts/submitted_at/last_log_at；3）巡检并将异常摘要写到 STATE.yaml 的 patrol_notes 字段。不需要唤醒 leader agent，也不要承担对群广播。"
```

### Cron 1 执行逻辑

```
1. 读 STATE.yaml + tasks-log.md

2. 自动同步：
   对每个非终态任务，找其最新一条 follower 日志：
   - 若日志 status ∈ {received, in-progress, blocked, submitted}
     且比 STATE.yaml 中的当前 status 更新 → 回写 STATE.yaml status 字段
   - 回写该条日志时间到 last_log_at
   - 提取 output 字段 → 回写 artifacts
   - 若日志 status = submitted → 同时回写 submitted_at
   （不修改任何终态：done / rejected / cancelled / failed）

3. 巡检并将结果写入 STATE.yaml patrol_notes（格式：分号分隔的 `<taskID>: <结论>` 对，如 `"R1: 日志停更; R2: blocked 等待决策"`）：
   对每个非终态任务：
   - status = assigned 且 3 分钟内仍无接单证据 → 标记"待确认：可能未接单"
   - status ∈ {received, in-progress, blocked} 且超过 deadline → 标记"执行超时"
   - status = submitted 且超过 accept_sla_min → 标记"待验收 SLA 超时"
   - status = blocked 且超过 10 分钟无新日志 → 标记"blocked 等待决策"
   - depends_on 未全部 done 但任务已 in-progress → 标记"依赖顺序异常"
   - 5 分钟无新日志且当前不是 submitted → 标记"日志停更"

4. 一切正常 → 清空 patrol_notes，不对群发消息
```

### 规则

- `submitted` 后，巡检关注点从执行 deadline 切换为 `accept_sla_min`。**submitted 不再参与执行超时报警。**
- `assigned` 阶段只看"是否收到接单证据"，不把 `sessions_send timeout` 直接当成失败。

## Cron 2 — 决策触发

**职责**：leader agent session，但只允许轻动作；先同步，再分流

```bash
openclaw cron add \
  --name "patrol-trigger-<slug>" \
  --cron "*/5 * * * *" \
  --tz "Asia/Shanghai" \
  --session main \
  --system-event "巡检触发：读 ~/.openclaw/tasks/<slug>/STATE.yaml，检查 patrol_notes 和非终态任务。若有需要处理的（submitted 待验收、blocked 待决策、日志停更、超时等），先按 STATE.yaml.broadcast 的 mode/channel/to 通过机器人向正式 IM 群发一条简短进展/异常同步，再执行轻动作；若当前会话不是该 IM 群对应的 OpenClaw 会话，不得把正式任务播报回当前会话，只能在必要时做一句解释。轻动作仅限：验收回写、sessions_send 催办/解锁、简短说明。任何预计超过 2 分钟的工作（例如接管补洞、长阅读、重写产物、整合文档）不得在当前群会话内直接做，必须立即改为 isolated helper / 子会话处理，然后再通过机器人回正式 IM 群报"已转后台处理 + ETA"。无需处理则忽略，不回复。" \
  --wake now
```

### Cron 2 触发后 leader agent 的动作

| STATE 中的情况 | leader agent 执行 |
|---|---|
| submitted 待验收 | 先发短同步 → 快速验收 → 回写 `done` 或 `rejected` + `rejection_note` |
| blocked 待决策 | 先发短同步 → 给出决策 → `sessions_send` 通知 follower 解锁继续 |
| 日志停更 / 3 分钟未接单 | 先发异常 → `sessions_send` 催办或确认；若需接管，立刻转 isolated helper |
| 依赖顺序异常 | 先发异常 → 核查后修正 STATE 或通知 follower 等待 |
| 需要接管/补洞的重活 | 先发异常或进展 → 立即转 isolated helper / 子会话 → 回群给 ETA |
| 无需处理 | 忽略，不回复 |

## Cron 删除（收尾时由 leader agent 执行）

```bash
openclaw cron remove <sync-jobId>
openclaw cron remove <trigger-jobId>
openclaw cron list
```

`cron remove` 的命令返回不算收尾完成。**只有再次执行 `openclaw cron list`，确认两个 jobId 都已消失，才算真正收尾。**
