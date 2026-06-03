本文件为 Agent 在此仓库中的最小工作约束。细节规范以 [docs/developer/design.md](docs/developer/design.md) 和 [docs/developer/development.md](docs/developer/development.md) 为准。

This file defines the minimal working constraints for agents in this repository. Detailed specifications are in [docs/developer/design.md](docs/developer/design.md) and [docs/developer/development.md](docs/developer/development.md).

## 项目 / Project

- `arc-kit` 是一个 Rust CLI，用于管理 coding agent 的 provider、skill、MCP、subagent 和 market。
  `arc-kit` is a Rust CLI for managing coding agent providers, skills, MCP, subagents, and markets.
- 目标平台仅为 macOS。
  Target platform is macOS only.
- Cargo workspace 主要分为 / Cargo workspace is divided into:
  - `arc-cli`：CLI、命令定义、用户输出 / CLI, command definitions, user output
  - `arc-core`：领域逻辑、状态与文件系统操作 / Domain logic, state, and filesystem operations
  - `arc-tui`：交互式终端 UI / Interactive terminal UI

## 必守规则 / Mandatory Rules

- 业务逻辑放在 `arc-core`，不要塞进 `arc-cli`。
  Business logic belongs in `arc-core`, not `arc-cli`.
- TUI / `dialoguer` 交互只放在 `arc-tui`。
  TUI / `dialoguer` interactions belong only in `arc-tui`.
- 所有行为变更必须带测试。
  All behavioral changes must include tests.
- 有代码变动时，必须同步更新 `README.md` 与 `docs/`。
  Code changes must be accompanied by updates to `README.md` and `docs/`.
- 不要混入无关重构；不要引入未使用依赖。
  No unrelated refactors; no unused dependencies.
- 代码注释与命令行提示使用英文；文档使用中英双语。
  Code comments and CLI prompts use English; documentation uses Chinese + English.

## CLI 语义 / CLI Semantics

- CLI 只有两类语义：**交互式** 与 **非交互式**。
  The CLI has only two semantic modes: **interactive** and **non-interactive**.
- 读取类命令必须支持 `--format json`（已登记例外除外）。
  Read commands must support `--format json` (registered exceptions excluded).
- 带向导的写入类命令必须提供非交互参数路径；非交互下不得读 stdin。
  Write commands with wizards must provide a non-interactive parameter path; non-interactive mode must not read stdin.
- JSON 输出不得混入 ANSI；退出码语义必须写入文档。
  JSON output must not contain ANSI; exit code semantics must be documented.
- `skill` / `mcp` / `subagent` 这类资源命令，按整组 `list / info / install / uninstall` 判断是否同时支持 for 人和 for Agent。
  Resource commands like `skill` / `mcp` / `subagent` are evaluated as a full `list / info / install / uninstall` family for both human and agent support.

交互与自动化细则见 [docs/developer/design.md](docs/developer/design.md)。

Details on interaction and automation are in [docs/developer/design.md](docs/developer/design.md).

## 验证 / Verification

- 提交前至少运行 / At minimum, run before committing:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

- 如果修改了 CLI 命令，还要补 / If CLI commands were modified, also run:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

- 发版前必须执行 / Must run before release:

```bash
./scripts/regression.sh
```

完整回归、黑盒矩阵和开发规范见 [docs/developer/development.md](docs/developer/development.md)。

Full regression, black-box matrix, and development conventions are in [docs/developer/development.md](docs/developer/development.md).

## 发版 / Release

- 先确认 `main` 推送成功，再单独打 tag 推送。
  Confirm `main` push succeeds, then push the tag separately.
- 不要执行 `git push origin main --tags`。
  Do not run `git push origin main --tags`.

## 风格 / Style

- 保持实现与交互简单、直接、可靠。
  Keep implementation and interaction simple, direct, and reliable.
