---
name: arc-cli-usage
description: >
  Use for any request about arc-kit or the `arc` CLI: installation, Homebrew, PATH, provider switching,
  skill install/uninstall/list/info, market add/remove/update, MCP install/define/uninstall, subagent install/uninstall,
  `arc.toml`, `arc project apply`, `arc project edit`, `arc status`, `--format json`, non-interactive/CI usage,
  project-vs-global capability rollout, or debugging why agent config did not land in Claude Code, Codex, Cursor CLI,
  OpenCode, Gemini CLI, Kimi CLI, or OpenClaw. Trigger even when the user only says things like
  「帮我配一下代理」, 「skill 没装上」, 「market 怎么加」, 「为什么 Codex/Claude 没生效」, 「arc 命令怎么用」,
  「why didn't project apply install」, or 「how do I manage MCP/subagents with arc」.
---

# arc-kit 使用指南

按下面顺序工作，保持回答短而准。

## 信息源

- 以 `docs/user/guide.md` 为主。
- 交互/非交互、JSON、退出码看 `docs/developer/design.md`。
- 若文档与实现冲突，以代码为准，尤其是 `arc-cli/src/cli.rs`、`arc-cli/src/commands/*`、`arc-core/src/agent/mod.rs`。

## 默认流程

1. 先判断用户是在问安装、查询、写入配置，还是排障。
2. 排障时先查状态，不要先写配置。
3. 只有在确认目标命令、scope 和 agent 后，才建议执行写入操作。

## 先查什么

- 总览：`arc status`
- provider：`arc provider` 或 `arc provider list`
- 已装 skill：`arc skill list --installed`
- MCP：`arc mcp list`
- subagent：`arc subagent list`
- market：`arc market list`

## 必须记住的语义

- 裸 `arc` 只会打印 `arc --help`，不等于 `arc status`。
- `--format json` 优先于 TTY。
- 写入类命令在非交互环境必须给显式参数，不能指望向导。
- 对写入类 JSON，除了退出码，还要检查 `ok`。
- `arc version` 不支持 JSON。
- `arc project edit` 只在交互式终端可用；非交互或 `--format json` 只返回失败的 `WriteResult`，不会执行编辑。

## 容易答错的项目级行为

- 项目级 skill 由 `arc.toml` + `arc project apply` 管理，不等于全局 `arc skill install`。
- Codex 的项目级 skill 路径是 `.agents/skills/`，不是 `.codex/skills/`。
- Codex 的项目级 MCP 和 subagent 路径在 `.codex/` 下。
- OpenClaw 不支持项目级 skill，也不参与 arc 管理的 MCP / subagent。
- Kimi CLI 的 MCP 仅支持全局配置；项目级声明会在 `arc project apply` 中跳过。如需让 Kimi 使用某个 MCP，应执行全局 `arc mcp install <name> --agent kimi`。
- Codex subagent 必须带非空 `description`。

## 能力速查

- project skill：
  Claude / Codex / Cursor / OpenCode / Gemini / Kimi 支持；OpenClaw 不支持。
- MCP：
  Claude / Codex / Cursor / OpenCode / Gemini / Kimi 支持；OpenClaw 不支持。
- subagent：
  只有 Claude / Codex / OpenCode 由 arc 原生写入；其余不要误答为已支持。
- 关键项目路径：
  Codex project skill 在 `.agents/skills/`；
  Codex project MCP / subagent 在 `.codex/`；
  Claude project MCP 在 `.mcp.json`；
  OpenCode project MCP 在项目根 `opencode.json`。

## 常见问法

- 「为什么 `arc project apply` 没生效」
  先看 `arc status`，再看仓库里是否真的有 `arc.toml`，再核对目标 agent 是否支持项目级能力，以及命令是否带了 `--agent` / `--all-agents`。
- 「为什么 skill 没装上」
  先区分全局还是项目级；全局看 `arc skill list --installed`，项目级看 `arc status` 和对应仓库内路径。
- 「为什么 MCP 没写进去」
  先区分 global / project，再核对目标 agent 是否支持该 transport 和 scope；若目标包含 Kimi，再确认是否误用了项目级声明。
- 「为什么 subagent 没生效」
  先确认目标 agent 是否在 Claude / Codex / OpenCode 之列；若包含 Codex，再检查是否给了非空 `description`。
- 「为什么 provider 切换后不对」
  先看 `arc provider list` / `arc provider test`，再核对对应 agent 的配置文件和 provider 类型差异，尤其是 Codex 的 `auth-only` / `proxy`。
- 「为什么脚本里拿不到结果」
  优先改用 `--format json`，并提醒用户读取写入类 JSON 的 `ok` 字段，而不是只看退出码。

## 常见失败信号

- 非交互环境缺少必要参数：
  这类通常直接失败，不会进入向导。
- `arc project edit`：
  非交互和 JSON 路径都不会打开编辑器，只会返回失败结果。
- `arc version`：
  没有 JSON 输出，不要让用户依赖 `--format json`。
- 用户手工改 agent 原生目录：
  不要默认 arc 一定会完整识别或维护；先区分「由 arc 跟踪的安装」与「手工放进去的内容」。
- `arc market update`：
  只会维护 arc 已追踪的全局 skill 安装，不要误答成「会清理所有 agent 目录里的同名 skill」。

## 回答时优先覆盖的点

- 用户是在用全局还是项目级配置。
- 用户是在交互式终端还是 CI / pipe / `--format json`。
- 目标 agent 是否真的支持该能力。
- 命令是读取类还是写入类。
- 若是排障，指出应先看哪条状态命令和哪条配置路径。
