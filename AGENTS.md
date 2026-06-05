This file defines the minimal working constraints for agents in this repository. Detailed specifications are in [docs/developer/design.md](docs/developer/design.md) and [docs/developer/development.md](docs/developer/development.md).

本文件为 Agent 在此仓库中的最小工作约束。细节规范以 [docs/developer/design.md](docs/developer/design.md) 和 [docs/developer/development.md](docs/developer/development.md) 为准。

## Project / 项目

- `arc-kit` is a Rust CLI for managing coding agent providers, skills, MCP, subagents, and markets.
  `arc-kit` 是一个 Rust CLI，用于管理 coding agent 的 provider、skill、MCP、subagent 和 market。
- Target platform is macOS only.
  目标平台仅为 macOS。
- Cargo workspace is divided into / Cargo workspace 主要分为:
  - `arc-cli`: CLI, command definitions, user output / CLI、命令定义、用户输出
  - `arc-core`: Domain logic, state, and filesystem operations / 领域逻辑、状态与文件系统操作
  - `arc-tui`: Interactive terminal UI / 交互式终端 UI

## Mandatory Rules / 必守规则

- Business logic belongs in `arc-core`, not in `arc-cli`.
  业务逻辑放在 `arc-core`，不要塞进 `arc-cli`。
- TUI / `dialoguer` interactions belong only in `arc-tui`.
  TUI / `dialoguer` 交互只放在 `arc-tui`。
- All behavioral changes must include tests.
  所有行为变更必须带测试。
- Code changes must be accompanied by updates to `README.md` and `docs/`.
  有代码变动时，必须同步更新 `README.md` 与 `docs/`。
- No unrelated refactors; no unused dependencies.
  不要混入无关重构；不要引入未使用依赖。
- Code comments and CLI prompts use English; documentation uses English + Chinese, with English first by default.
  代码注释与命令行提示使用英文；文档使用英文+中文，默认英文在前。

## CLI Semantics / CLI 语义

- The CLI has only two semantic modes: **interactive** and **non-interactive**.
  CLI 只有两类语义：**交互式** 与 **非交互式**。
- Read commands must support `--format json` (registered exceptions excluded).
  读取类命令必须支持 `--format json`（已登记例外除外）。
- Write commands with wizards must provide a non-interactive parameter path; non-interactive mode must not read stdin.
  带向导的写入类命令必须提供非交互参数路径；非交互下不得读 stdin。
- JSON output must not contain ANSI; exit code semantics must be documented.
  JSON 输出不得混入 ANSI；退出码语义必须写入文档。
- Resource commands like `skill` / `mcp` / `subagent` are evaluated as a full `list / info / install / uninstall` family for both human and agent support.
  `skill` / `mcp` / `subagent` 这类资源命令，按整组 `list / info / install / uninstall` 判断是否同时支持 for 人和 for Agent。

Details on interaction and automation are in [docs/developer/design.md](docs/developer/design.md).

交互与自动化细则见 [docs/developer/design.md](docs/developer/design.md)。

## Verification / 验证

- At minimum, run before committing / 提交前至少运行:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

- If CLI commands were modified, also run / 如果修改了 CLI 命令，还要补:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

- Must run before release / 发版前必须执行:

```bash
./scripts/regression.sh
```

Full regression, black-box matrix, and development conventions are in [docs/developer/development.md](docs/developer/development.md).

完整回归、黑盒矩阵和开发规范见 [docs/developer/development.md](docs/developer/development.md)。

## Release / 发版

- Confirm `main` push succeeds, then push the tag separately.
  先确认 `main` 推送成功，再单独打 tag 推送。
- Do not run `git push origin main --tags`.
  不要执行 `git push origin main --tags`。

## Style / 风格

- Keep implementation and interaction simple, direct, and reliable.
  保持实现与交互简单、直接、可靠。
