## 多 Agent 协作机制 — Follower 协议

> 你的目标不是显得忙，是产出可验收的结果。

进入本文前，先确认当前请求需要你以 **follower** 身份执行自己收到的子任务。只要是接单、执行、写日志、提交、blocked 汇报、被退回后重做，就按本文执行；如果是在开局建盘、巡检、验收、改派、补洞或收尾，切到 [leader-protocol.md](leader-protocol.md)。

这里的"群"默认指 **IM 群**（Feishu / Telegram 等），不是 OpenClaw session。

### 角色

```
老板（人类）
  └── leader agent — 规划、分派、验收、兜底
        ├── 你 — 执行、产物、日志
        └── 其他 follower agent
```

leader agent 对最终交付负全责。你对自己的子任务产物负责。

### 文件权限

- `GOALS.md` / `STATE.yaml` / `DECISIONS.md`：**只读**，不允许写入任何内容
- `tasks-log.md`：**只能通过脚本追加**，禁止直接编辑（并发写入会破坏日志完整性）
- `artifacts/<你的名字>/<taskID>/`：**读写**，你的专属产物目录
- `artifacts/<其他人>/`：**只读**，需要引用时只读，不要修改

### 执行流程

**1. 接单（3 分钟内）**
- 在对应的 IM 群里回复：`收到 <task-id>，理解为：<一句话>，预计 <时间> 前交付`
- 写 `received` 日志
- 若同一个 `task-id` 被重复分派，先回报当前状态：`已收` / `进行中` / `已提交等待验收`，**不要**静默重开一份

**2. 执行**
- 读 GOALS.md + STATE.yaml + DECISIONS.md（只读，不改）
- 检查 `depends_on`：依赖的任务未 `done` 之前不要开始，先通知 leader agent 确认
- 产物写到 `artifacts/<你的名字>/<taskID>/v1-<name>.md`
- 有进展写 `in-progress` 日志；遇到阻塞立刻写 `blocked` 日志 + 通知 leader agent（卡点、影响、建议）
- 5 分钟没日志 = cron 会催你 = leader agent 可能改派你

**3. 提交**
- 自检：产物存在 ✅ 格式正确 ✅ 结论有证据 ✅
- 写 `submitted` 日志
- 在对应的 IM 群里发：`<task-id> submitted，关键结论：<1-3条>，产物：<路径>`
- `submitted` 的含义是"等待 leader agent 验收"，不是"任务已经 done"
- `done` 只能由 leader agent 验收后回写，你不能自行标记
- 如果 leader agent 在你已提交后再次催办，直接回：`<task-id> 已提交，等待验收，产物：<路径>`

**4. 被退回（rejected）**
- 读 STATE.yaml 中的 `rejection_note`，理解退回原因
- 在新版本子目录重做：`artifacts/<你的名字>/<taskID>/v2-<name>.md`（**不覆盖**旧版）
- 重新走步骤 2 → 3

**阻塞时：** 写 `blocked` 日志 + 通知 leader agent。等决策，不自行绕过。若确认无法完成，在 blocked 日志中注明，由 leader agent 决定是否标记 `failed`。

**关键语义：**
- `submitted` 后你进入"等待验收"状态，不再按执行 deadline 自己判断是否超时。
- 如果发现分派信息和当前 `STATE.yaml` 冲突，先报冲突，再等 leader agent 定口径，不自己改状态。
- 正式对外可见的同步以 IM 群为准，不要把 OpenClaw 当前会话误当成正式广播出口。

### 写日志（必须用脚本）

脚本位于 [scripts/task_log_append.py](../scripts/task_log_append.py)，执行时用完整路径：

```bash
python3 ~/.claude/skills/claw-multi-agent-collab/scripts/task_log_append.py \
  --task-dir ~/.openclaw/tasks/<slug> \
  --agent <你的名字> \
  --task <task-id> \
  --status <received|in-progress|blocked|submitted> \
  --summary "一句话" \
  --output "artifacts/<你的名字>/<taskID>/v1-report.md" \
  --next "下一步或 ISO 8601 时间" \
  --blockers "none"
```

时间格式统一用 ISO 8601 含时区 offset：`2026-03-13T22:40:00+08:00`，不用 `CST` 等歧义缩写。

脚本失败（exit code 1）时自动重试 3 次；仍失败则通知 leader agent。

### 日志格式示例

```
- 2026-03-13T22:40:00+08:00 | agent=two | task=R1 | status=received
  - summary: 收到任务，预计 23:00 前交付
  - output: n/a
  - next: 2026-03-13T22:50:00+08:00
  - blockers: none
- 2026-03-13T22:55:00+08:00 | agent=two | task=R1 | status=in-progress
  - summary: 已完成框架搭建
  - output: artifacts/two/R1/v1-draft.md
  - next: 2026-03-13T23:10:00+08:00
  - blockers: none
- 2026-03-13T23:10:00+08:00 | agent=two | task=R1 | status=submitted
  - summary: 结果已提交，等待 leader agent 验收
  - output: artifacts/two/R1/v1-report.md
  - next: leader-agent-acceptance
  - blockers: none
```

### 红线

- 不改 GOALS / STATE / DECISIONS / 其他人的产物
- 没自检不报 submitted
- 不伪造结论（每条结论至少有一个证据）
- 不自行把任务标记为 done 或 failed
- 不因 `sessions_send` 重复触达就私自重开同一任务
