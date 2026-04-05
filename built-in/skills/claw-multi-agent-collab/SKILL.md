---
name: claw-multi-agent-collab
description: |
  OpenClaw 多 agent 协作与 leader/follower 执行协议。只要用户想在 OpenClaw 里拆任务、分派子 agent、起双 cron、做巡检、催办、退回重做、修正正式广播路由、或在多 agent 任务里接管补洞，就主动使用此 skill。即使用户没有明确说"leader / follower / 多 agent 协议"，只要需求本质上是在推进 `~/.openclaw/tasks/<slug>/` 下的协作任务，也应触发；反过来，通用团队协作、纯架构设计、普通 cron/YAML 问题不要误触发。
---

# OpenClaw 多 Agent 协作机制

这个 skill 不是给你复述协议用的，而是让你在 OpenClaw 场景里先判角色，再给出可执行行动单，直接把任务往前推。

## 适用范围

在这些场景中使用本 skill：

- 用户要在 OpenClaw 里创建或推进一个多 agent 任务
- 用户要把任务分给 `two` / `three` / `four` 等 follower，并要求 leader 验收
- 用户提到 `~/.openclaw/tasks/<slug>/`、`STATE.yaml`、`tasks-log.md`、`sessions_send`、双 cron、`broadcast` 路由
- 用户在处理 submitted 待验收、blocked 待决策、日志停更、改派、退回重做、接管补洞
- 用户要求把正式播报纠正回 IM 群，而不是当前 OpenClaw 会话

这些情况不要使用本 skill：

- 只是讨论通用 multi-agent 架构、数据库设计、Jira 拆任务、协作方法论
- 只是问 cron 表达式、普通 YAML 语法、一般性的群机器人配置
- 只是让你写一份业务文档，但没有 OpenClaw 任务协作上下文

## 环境前提

默认这是一个 **OpenClaw 专用** skill。使用时假定以下约束成立：

- 任务目录固定为 `~/.openclaw/tasks/<slug>/`
- 正式广播目标来自 `STATE.yaml.broadcast`
- 任务分派通过 `sessions_send`
- 巡检采用双 cron：先 sync，再 trigger
- 日志脚本固定路径：`~/.claude/skills/claw-multi-agent-collab/scripts/task_log_append.py`（随 skill 分发）

如果用户的环境不满足这些前提，不要硬套本协议。明确说明缺少 OpenClaw 上下文，并改为给出边界说明或请用户补充环境信息。

## 默认响应契约

只要本 skill 被触发，默认按下面顺序回答。用户如果要求你直接执行，就照这个结构推进，不要只停留在"协议摘要"。

### 角色判断

先判断你当前是在扮演哪一个角色，并给一句判定依据：

- `Leader`：开局建盘、分派、起双 cron、巡检、验收、退回、改派、补洞、收尾
- `Follower`：接单、执行、写日志、提交、被退回后重做
- `Leader / 巡检异常`：submitted 待验收、blocked 待决策、日志停更、deadline 超时、广播路由错误

如果请求混合多个角色：

- 先选当前要行动的主角色
- 再补充另一个角色需要遵守的约束
- 不要把 leader 和 follower 的写权限混在一起

### 输出结构

除非用户只要一句话，否则尽量使用这 5 段标题：

```markdown
### 角色判断
### 立即动作
### 需要读取 / 更新的文件
### 命令 / 消息模板
### 风险与下一检查点
```

输出要求：

- `立即动作` 用 3-7 步行动单，按先后顺序写
- `命令 / 消息模板` 必须给具体命令、路径、消息模板，而不是抽象描述
- 用户要求你直接干活时，直接生成文件骨架、分派消息、巡检命令、验收回写草稿等可交付内容
- 明确剩余未完成项和下一检查点，不要假装任务已经 `done`

## 角色路由

| 用户当前要推进的事 | 你应采用的角色 | 先读什么 |
|---|---|---|
| "帮我拆任务 / 分派 / 起巡检 / 开工" | Leader | [references/leader-protocol.md](references/leader-protocol.md) + [references/state-machine.md](references/state-machine.md) |
| "我收到了任务 / 我要提交 / 我卡住了" | Follower | [references/follower-protocol.md](references/follower-protocol.md) |
| "日志停更 / deadline 过了 / submitted 待验收 / 广播发错地方" | Leader | [references/leader-protocol.md](references/leader-protocol.md) |
| "STATE.yaml 现在该怎么回写 / 某状态是不是合法" | 按当前动作选角色，再读状态机 | [references/state-machine.md](references/state-machine.md) |
| "双 cron 怎么建 / 怎么删 / 为什么没开工" | Leader | [references/cron-automation.md](references/cron-automation.md) + [references/leader-protocol.md](references/leader-protocol.md) |

如果你发现请求本质上是 OpenClaw leader 在做决策，不要因为用户用了"我该怎么办"这类口语就误判成 follower。

## Leader 入口

以下情况默认走 leader 入口：

- 新建任务盘面
- 分派 follower 任务
- 起双 cron 并设置广播
- 处理异常、submitted、rejected、改派、接管补洞
- 收尾并删除 cron

推进顺序：

1. 先读 [references/leader-protocol.md](references/leader-protocol.md)
2. 涉及状态写回或字段合法性时补读 [references/state-machine.md](references/state-machine.md)
3. 涉及双 cron 创建或删除时补读 [references/cron-automation.md](references/cron-automation.md)
4. 需要真正建盘时，优先使用 `templates/` 下的模板文件做骨架

leader 回答时默认要产出：

- 当前任务是 leader 视角的明确判断
- 任务目录、文件骨架和关键字段
- `sessions_send` 分派消息
- 双 cron 命令或巡检动作
- 必要的群同步消息

leader 红线：

- 双 cron 没成功创建，不得宣布 `【开工】`
- 正式广播只能走 `STATE.yaml.broadcast` 指向的 IM 群，不得拿当前 OpenClaw 会话冒充正式播报
- follower 的事实性状态来自日志和 cron；你只手动写判断性状态
- 需要重活时先对群报"已转后台处理 + ETA"，再转 isolated helper / 子会话，不要在错误会话硬做长活

## Follower 入口

以下情况默认走 follower 入口：

- 刚收到 leader 分派的子任务
- 正在执行、写日志、提交产物
- 遇到阻塞，要写 `blocked` 并向 leader 汇报
- 被退回后要在新版本目录重做

推进顺序：

1. 先读 [references/follower-protocol.md](references/follower-protocol.md)
2. 如需确认状态含义或 `rejection_note` 语义，再读 [references/state-machine.md](references/state-machine.md)
3. 产物路径、日志命令、接单/提交消息都直接给到，不要只说"按协议处理"

follower 回答时默认要产出：

- 接单或提交的 IM 群消息模板
- [scripts/task_log_append.py](scripts/task_log_append.py) 的具体命令
- 产物应写入的目录与版本号
- 当前不能做的事，例如不能自行写 `done` / `failed`

follower 红线：

- 不改 `GOALS.md` / `STATE.yaml` / `DECISIONS.md`
- 不直接编辑 `tasks-log.md`，必须走脚本
- `submitted` 只是等待 leader 验收，不等于任务已完成
- 被退回时在 `v2` / `v3` 新版本文件里重做，不覆盖旧稿

## 巡检 / 异常入口

这些情况按 leader 处理，不要误判成"只是解释一下"：

- `sessions_send timeout`
- 3 分钟未接单
- 5 分钟无新日志
- `blocked` 超过 10 分钟待决策
- `submitted` 超过 `accept_sla_min`
- 正式广播发到了错误会话

默认动作：

1. 先查 `STATE.yaml`、`tasks-log.md`、正式 IM 群里的接单或提交证据
2. 判断是催办、解锁、改派，还是接管补洞
3. 先发简短 `【进展】` 或 `【异常】`，再做轻动作
4. 若需要长时间工作，立即转后台处理并给 ETA

异常处理时，明确提醒这些规则：

- `sessions_send timeout` 不等于未送达
- 日志停更先核查证据，再决定催办或接管
- 若已有 70% 内容且只剩少数分支挂住，leader 直接补洞，交付优先

## 验收 / 退回 / 补洞入口

这些情况也按 leader 处理：

- follower 已 `submitted`，需要 spot check
- 产物证据不足、格式不符、需要 `rejected`
- 某分支掉线，但整体内容已基本可交付，需要 leader 补洞
- 全部终态后要发 `【完成】` 并删 cron

默认动作：

1. 先快速 spot check：事实、证据、格式、路径、版本
2. 验收通过则回写 `done`
3. 验收不通过则回写 `rejected` + `rejection_note`
4. 要求 follower 在新版本目录重做，不覆盖旧稿
5. 只有最终整合完成并已正式对群发 `【完成】<slug>`，才允许删除 cron

## 模板与脚本

真的要开工时，优先用这些现成骨架：

- [templates/GOALS.template.md](templates/GOALS.template.md)
- [templates/STATE.template.yaml](templates/STATE.template.yaml)
- [templates/DECISIONS.template.md](templates/DECISIONS.template.md)
- [templates/tasks-log.template.md](templates/tasks-log.template.md)

真的要执行时，使用技能内置脚本（安装后在 `./scripts/`）：

- [scripts/task_log_append.py](scripts/task_log_append.py)：追加 follower 日志（follower 写 `tasks-log.md` 的唯一合法方式）

## 共享硬规则

- 任务目录只允许在 `~/.openclaw/tasks/<slug>/` 下
- [references/state-machine.md](references/state-machine.md) 是状态口径的唯一权威来源
- 正式广播看 `broadcast.channel + broadcast.to`，不是当前聊天会话
- 重版本永远新建 `v2` / `v3`，不要覆盖 `v1`
- follower 只写日志和自己的 `artifacts/<agent>/<taskID>/`
- leader 没发正式 `【完成】` 之前，不得删除巡检 cron

## 参考文档路由

| 主题 | 文件 | 什么时候读 |
|---|---|---|
| Leader 完整职责 | [references/leader-protocol.md](references/leader-protocol.md) | 需要建盘、分派、巡检、验收、退回、收尾 |
| Follower 完整流程 | [references/follower-protocol.md](references/follower-protocol.md) | 需要接单、执行、日志、提交、阻塞处理 |
| 状态机与 `STATE.yaml` | [references/state-machine.md](references/state-machine.md) | 需要判断状态合法性、字段含义、回写规则 |
| Cron 自动化 | [references/cron-automation.md](references/cron-automation.md) | 需要创建、排查、删除双 cron |
