# Tasks Log

<!-- Follower agent 通过脚本追加日志，禁止直接编辑此文件 -->
<!-- 使用命令：python3 ~/.claude/skills/claw-multi-agent-collab/scripts/task_log_append.py ... -->

<!-- 接单确认 -->
- [ISO 8601 时间] | agent=[agent-name] | task=[task-id] | status=received
  - summary: [一句话描述]
  - output: n/a
  - next: [下一步或 ISO 8601 时间]
  - blockers: none

<!-- 执行进展 -->
- [ISO 8601 时间] | agent=[agent-name] | task=[task-id] | status=in-progress
  - summary: [一句话描述]
  - output: [产物路径，如 artifacts/two/R1/v1-draft.md]
  - next: [下一步或 ISO 8601 时间]
  - blockers: none

<!-- 提交等待验收 -->
- [ISO 8601 时间] | agent=[agent-name] | task=[task-id] | status=submitted
  - summary: [一句话描述]
  - output: [产物路径，如 artifacts/two/R1/v1-report.md]
  - next: leader-agent-acceptance
  - blockers: none

<!-- 阻塞（如有） -->
- [ISO 8601 时间] | agent=[agent-name] | task=[task-id] | status=blocked
  - summary: [阻塞原因]
  - output: n/a
  - next: [leader agent 决策后继续]
  - blockers: [具体阻塞点]
