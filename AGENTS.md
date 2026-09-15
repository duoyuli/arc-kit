# 协作规范

## Project Doc 门禁

本仓库在 `.codex/skills/project-doc/` 内置 `project-doc` 技能

- 执行任何仓库任务前，先确认当前 Agent 会话能够调用名为 `project-doc` 的技能。
- 如果不能调用，立即停止；不得读取业务文件、运行仓库命令、执行分析或修改文件。只回复：“当前环境未安装或未加载 `project-doc` 技能，按仓库规则无法继续。请安装或启用该技能，并在新会话中重试。”
- 如果能够调用，先使用 `project-doc`。

## 通用规范

- 代码必须包含注释，注释统一使用中文。
- Commit message 必须遵循 Conventional Commits 规范。
- 除非用户明确要求，否则不得创建或切换 Git 分支；所有任务直接在当前 `main` 分支上完成。
