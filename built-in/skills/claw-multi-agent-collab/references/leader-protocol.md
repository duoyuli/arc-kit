## 多 Agent 协作机制 — Leader 协议

> 交付结果，不是管理流程。核心是：建团队、定策略、拿结果。

进入本文前，先确认当前请求需要你以 **leader** 身份决策或推进。只要是在开局建盘、分派、巡检、异常处理、验收、退回、广播纠偏、接管补洞、收尾，就按本文执行；如果只是指导一个 follower 完成自己收到的子任务，切到 [follower-protocol.md](follower-protocol.md)。

### 一、角色

- **leader agent**：规划、分派、验收、兜底。对最终交付负全责。
- **follower agent**：执行、产出、汇报。对自己的子任务产物负责。

### 二、任务目录

根路径：`~/.openclaw/tasks/<slug>/`

```
├── GOALS.md        # 目标 + 验收标准（leader agent 写）
├── STATE.yaml      # 当前任务状态真相（leader agent + cron 维护，见第四节）
├── DECISIONS.md    # 关键决策记录（leader agent 维护）
├── tasks-log.md    # 事件日志（follower agent 追加）
└── artifacts/      # 产物
    └── <agent>/
        └── <taskID>/
            ├── v1-report.md
            └── v2-report.md   # 被退回重做后新增，不覆盖旧版
```

**权责：**
- leader agent 维护 GOALS / DECISIONS；
- STATE.yaml 由 leader agent 写判断性状态、由 cron 自动同步事实性状态（见第四节）；
- follower agent 只追加日志、写自己的 `artifacts/<名字>/<taskID>/` 子目录，互不越界。

**禁止在 `workspace` 或 `workspace-*` 下创建 `tasks/` 目录**，统一用 `~/.openclaw/tasks/`。

### 三、状态机 + STATE.yaml

状态机定义、状态转换规则、STATE.yaml schema 及字段说明，请参阅 [state-machine.md](state-machine.md)。

**leader agent 需要掌握的关键点：**
- 你是唯一能设置终态（`done` / `rejected` / `cancelled` / `failed`）的角色
- 事实性状态（`received` / `in-progress` / `blocked` / `submitted`）由 Cron 1 自动从日志回写
- `rejected` 时必须填 `rejection_note`，follower 重做时会读取
- 分派任务前，确保 `depends_on` 中的依赖已全部 `done`

### 四、STATE 自动同步 + 巡检 Cron

双 cron 机制的完整定义（创建命令、执行逻辑、巡检规则、删除流程）请参阅 [cron-automation.md](cron-automation.md)。

**leader agent 需要掌握的关键点：**
- **硬门槛**：双 cron 没成功创建，不算开工。先建 `patrol-sync-<slug>`，再建 `patrol-trigger-<slug>`，拿到两个 jobId 回写 `STATE.yaml.automation`，然后才能发 `【开工】`
- Cron 1（3 分钟，isolated）：自动同步事实性状态 + 巡检写 `patrol_notes`
- Cron 2（5 分钟，leader session）：读 `patrol_notes` 后执行轻动作（验收、催办、解锁）；重活转 isolated helper
- 收尾时先删 cron，再用 `openclaw cron list` 复核双 job 已消失

### 五、分派

通过 `sessions_send` 发给 follower agent，简报包含：

```
【分派】<slug>/<task-id>
背景：一句话
任务：边界清晰的描述
输入：已有资料/文件路径
输出：写到 artifacts/<agent名>/<taskID>/v1-xxx.md
截止：ISO 8601 时间点
依赖：<task-id> 已完成 / 无
```

分派后，**只有在双 cron 已成功创建后**，才能在群里发开工通知：

```
【开工】<slug>
分工：R1 two / R2 three / R3 four
截止：<时间>
下次同步：<时间>
```

**`sessions_send timeout` 处理规则：**
- timeout 只表示：这次调用窗口内没等到回复。
- timeout **不等于** 没送达，也**不等于** 对方掉线。
- 遇到 timeout，leader agent 先查三样：
  - `tasks-log.md` 是否已出现 `received` / `in-progress` / `submitted`
  - 正式广播群里是否已出现 follower 接单确认
  - 是否已有 inter-session 回执或后续补报
- 在没有反证前，只能标记为"待确认"，**不能**直接判失败后重复分派。

### 六、验收

follower 的 `submitted` 只是"交稿"，不是"可直接并入最终交付"。

leader agent 至少做一轮快速 spot check，再决定 `done` / `rejected`：
- 核心事实是否正确
- 结论是否与输入材料、盘面或上下文一致
- 是否存在明显自相矛盾、口径漂移、漏掉关键风险
- 产物路径是否正确，是否引用了最新版文件

验收不通过时：
- 回写 `rejected`
- 填 `rejection_note`
- 明确要求 follower 在 `v2` / `v3` 新版本文件中重做，不覆盖旧稿

### 七、群同步

**广播路由硬规则：**
- 任务同步的唯一正式出口是 `STATE.yaml.broadcast`。
- `broadcast` 代表的是 **IM 群目标**，不是 OpenClaw session。正式广播必须通过机器人按 `channel + to` 发到 IM 群。
- 只要 `broadcast` 已定义，leader agent 就**不得**把"【开工】/【进展】/【异常】/【完成】"发到当前 OpenClaw 会话来代替正式广播。
- 当前会话如果刚好映射到同一个 IM 群，可以在当前会话补一句；但正式口径仍以机器人发到 IM 群为准。
- 私聊、排障会话、临时控制会话里，最多只允许发一句解释（例如"正式播报已改走群机器人"），不能持续承接任务播报。
- Cron / 巡检 / 子会话提醒若落在错误会话，leader agent 的第一动作是**纠正 IM 广播目标**，不是顺手在错误会话继续汇报。
- `broadcast.to` 必须是 IM 群标识，不是 OpenClaw session key。

**只有 3 种消息：**

**1. 进展（定期 + 里程碑时）**
```
【进展】<slug>
📝 R1 two submitted（待验收）
🔄 R2 three in-progress
⏳ R3 four received
下次同步：<时间>
```

**2. 异常（阻塞/掉线/改派时，立即发）**
```
【异常】<slug>
R2 three 日志停更 30min，已催办
```

**3. 完成（仅 leader agent 验收后）**
```
【完成】<slug>
结果：1-3 行总结
详情：~/.openclaw/tasks/<slug>/
```

**节奏：** 任务进行中，每 20 分钟至少同步一次。没有新产物也要发"仍在推进"。异常立即发。

### 八、follower agent 群消息

**只需 2 种：**

**接单：**
```
收到 <task-id>，理解为：<一句话>，预计 <时间> 前交付
```

**提交：**
```
<task-id> submitted，关键结论：<1-3条>，产物：<路径>
```

阻塞时写日志 + 通知 leader agent 即可，不需要发模板。

### 九、异常处理

| 触发 | 动作 |
|---|---|
| 3 分钟未确认接单 | 先查日志 / 群确认 / 回执；仍无证据再重发一次，必要时换人或接管 |
| `sessions_send timeout` | 标记"待确认"，先核查送达证据，禁止直接按失败重派 |
| 5 分钟无日志 | 催办，无回应 → 改派或接管 |
| submitted 超过 `accept_sla_min` | 立即提醒 leader agent 验收回写 STATE |
| blocked 超过 10分钟 无跟进 | 提醒 leader agent 做决策（解锁 / 取消 / 确认 failed） |
| `received` / `in-progress` / `blocked` 超过 deadline | 延期预警，改派或补洞 |
| 进程挂了 | 立即接管 |
| 产物跑题 / 格式不符 | 退回（写 `rejected` + `rejection_note`）或直接修 |
| follower 申请 failed | leader agent 评估：确认写 `failed`，或退回继续 |

**兜底原则：** 70% 内容已有，少数分支挂了 → leader agent 直接补洞出完整版。交付优先。

### 十、收尾

**硬规则：子任务全 done，不等于任务整体结束。**
- 只要 leader agent 还没完成最终整合、还没通过机器人向 `broadcast.channel + broadcast.to` 发出正式 `【完成】<slug>`，就**不允许**删除巡检 cron。
- 即使所有 follower 都已 done，只要总汇还没发，任务仍处于"收口中"，不是"已完成"。

收尾顺序固定为：
1. 所有子任务均为终态（`done` / `cancelled` / `failed`）
2. leader agent 完成最终整合，并通过机器人向 `broadcast.channel + broadcast.to` 发出正式 `【完成】<slug>` 通知
3. 再删除巡检 cron，并用 `openclaw cron list` 复核双 job 已消失
4. 有价值的决策记录写入 memory

### 十一、自检工具

**开工前验 STATE 合法性（手动检查替代）：**

读取 `STATE.yaml` 确认以下字段完整且合法：
- `tasks[]` 每项有 `id / status / agent / deadline`
- 所有 `status` 值为合法状态机值（见 [state-machine.md](state-machine.md)）
- `broadcast.channel` 和 `broadcast.to` 已填且指向 IM 群，不是 OpenClaw session key
- `automation.patrol_sync_job_id` 和 `automation.patrol_trigger_job_id` 已回写

**收尾前验双 cron 是否清干净：**

```bash
openclaw cron list
```

确认 `patrol-sync-<slug>` 和 `patrol-trigger-<slug>` 两个 jobId 均已从列表消失。
