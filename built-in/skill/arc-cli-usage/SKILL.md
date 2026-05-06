---
name: arc-cli-usage
description: >
  Use for any request about arc-kit or the `arc` CLI: installation, Homebrew, PATH, provider switching,
  skill install/uninstall/list/info, market add/remove/update, `arc.toml`, `arc project apply`,
  `arc project edit`, `arc status`, `--format json`, non-interactive/CI usage, project-vs-global
  skill rollout, or debugging why agent config did not land in Claude Code, Codex, Cursor CLI,
  OpenCode, Gemini CLI, Kimi CLI, or OpenClaw.
---

# arc-kit 使用指南

按下面顺序工作，保持回答短而准。

## 信息源

- 以 `docs/user/guide.md` 为主。
- 交互/非交互、JSON、退出码看 `docs/developer/design.md`。
- 若文档与实现冲突，以代码为准，尤其是 `arc-cli/src/cli.rs`、`arc-cli/src/commands/*`、`arc-core/src/agent/mod.rs`。

## 先查什么

- 总览：`arc status`
- provider：`arc provider` 或 `arc provider list`
- 已装 skill：`arc skill list --installed`
- market：`arc market list`

## 必须记住的语义

- 裸 `arc` 只会打印 `arc --help`，不等于 `arc status`。
- `--format json` 优先于 TTY。
- 写入类命令在非交互环境必须给显式参数，不能指望向导。
- 对写入类 JSON，除了退出码，还要检查 `ok`。
- `arc version` 不支持 JSON。
- `arc project edit` 只在交互式终端可用；非交互或 `--format json` 只返回失败的 `WriteResult`，不会执行编辑。
- MCP 与 subagent 管理功能已移除；不要建议 `arc mcp`、`arc subagent`、`[mcps]` 或 `[subagents]`。

## 容易答错的项目级行为

- 项目级 skill 由 `arc.toml` + `arc project apply` 管理，不等于全局 `arc skill install`。
- Codex 的项目级 skill 路径是 `.codex/skills/`。
- OpenClaw 不支持项目级 skill。
- `arc.toml` 只支持 `version`、`provider`、`skills` 和 `markets`。

## 能力速查

- project skill：
  Claude / Codex / Cursor / OpenCode / Gemini / Kimi 支持；OpenClaw 不支持。
- 关键项目路径：
  Codex project skill 在 `.codex/skills/`；
  Claude project skill 在 `.claude/skills/`；
  Cursor project skill 在 `.cursor/skills/`；
  OpenCode project skill 在 `.opencode/skills/`；
  Gemini project skill 在 `.gemini/skills/`；
  Kimi project skill 在 `.kimi/skills/`。

## 常见问法

- 「为什么 `arc project apply` 没生效」
  先看 `arc status`，再看仓库里是否真的有 `arc.toml`，再核对目标 agent 是否支持项目级 skill，以及命令是否带了 `--agent` / `--all-agents`。
- 「为什么 skill 没装上」
  先区分全局还是项目级；全局看 `arc skill list --installed`，项目级看 `arc status` 和对应仓库内路径。
- 「为什么 provider 切换后不对」
  先看 `arc provider list` / `arc provider test`，再核对对应 agent 的配置文件和 provider 类型差异。
- 「为什么脚本里拿不到结果」
  优先改用 `--format json`，并提醒用户读取写入类 JSON 的 `ok` 字段，而不是只看退出码。

## 常见失败信号

- 非交互环境缺少必要参数：这类通常直接失败，不会进入向导。
- `arc project edit`：非交互和 JSON 路径都不会打开编辑器，只会返回失败结果。
- `arc version`：没有 JSON 输出，不要让用户依赖 `--format json`。
- 用户手工改 agent 原生目录：不要默认 arc 一定会完整识别或维护；先区分「由 arc 跟踪的安装」与「手工放进去的内容」。
- `arc market update`：只会维护 arc 已追踪的全局 skill 安装，不要误答成「会清理所有 agent 目录里的同名 skill」。
