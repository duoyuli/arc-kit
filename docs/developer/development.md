# 开发与贡献 / Development & Contribution

命令与交互设计见 [用户手册](../user/guide.md) 与 [交互与自动化设计](design.md)。仓库里的 agent 支持、项目级路径和内置资源命名，都以当前代码实现为准。

Command and interaction design: [user manual](../user/guide.md) and [interactive/non-interactive design](design.md). Agent support, project-level paths, and built-in resource naming in this repo are authoritative per the current code.

## 贡献方式 / How to Contribute

- 缺陷：提交 Issue，写清复现步骤、预期/实际行为、系统与已安装的 coding agent。
  Bugs: file an Issue with repro steps, expected/actual behavior, OS, and installed coding agents.
- 较大功能或重构：先讨论范围再动手。
  Major features/refactors: discuss scope first.
- 行为变更：须补测试，覆盖改动到的核心路径。
  Behavioral changes: must include tests covering affected core paths.

## 开发环境 / Development Environment

- Rust：稳定版 toolchain / Stable toolchain
- 平台：当前以 macOS 为主 / Platform: macOS primary

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

## 提交前检查 / Pre-commit Checks

仓库根目录执行 / Run at repo root:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

若改动 CLI 入口、输出格式或交互语义，须补充黑盒检查 / If CLI entry, output format, or interaction semantics changed, also run:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

## 发版前完整回归 / Pre-release Full Regression

版本号变更、打 `v*` tag 或正式发布前，必须通过 / Before version bumps, `v*` tags, or formal releases, must pass:

```bash
./scripts/regression.sh
```

脚本内容是 `cargo fmt --all --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，以及在隔离的 `ARC_KIT_USER_HOME` 下执行 CLI 黑盒。覆盖内容包括：

The script runs `cargo fmt --all --check`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and CLI black-box tests in an isolated `ARC_KIT_USER_HOME`. Coverage includes:

- `status --format json` 的模块存在性（`project`、`agents`、`catalog`、`actions`）
  Module presence in `status --format json` (`project`, `agents`, `catalog`, `actions`)
- `skill install`、`skill uninstall`、`provider use` 在 `--format json` 下的非交互缺参失败
  Non-interactive missing-param failures for `skill install`, `skill uninstall`, `provider use` with `--format json`
- `skill info` 的结构化 JSON 错误
  Structured JSON errors from `skill info`
- `mcp` / `subagent` 命令已移除
  `mcp` / `subagent` commands removed
- `[mcps]` 等移除后的 `arc.toml` section 会被拒绝
  Removed sections like `[mcps]` are rejected in `arc.toml`

## 仓库结构 / Repository Structure

```text
.
├── arc-cli/          # CLI、clap、用户输出、format JSON
│                     # CLI, clap, user output, format JSON
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、detect
│                     # Domain logic, install engine, provider, market, skill, detect
├── arc-tui/          # 交互 UI（仅本 crate 依赖 dialoguer）
│                     # Interactive UI (only this crate depends on dialoguer)
├── built-in/         # 内置 skill 与 market 索引
│                     # Built-in skills and market index
├── docs/             # 官方文档 / Official documentation
├── scripts/
│   └── regression.sh # 发版前回归 / Pre-release regression
└── Cargo.toml
```

## 模块职责 / Module Responsibilities

- `arc-cli`：`app` 编排、`cli` 命令表、`commands/*`、`format.rs` JSON 结构体。
  `arc-cli`: `app` orchestration, `cli` command table, `commands/*`, `format.rs` JSON structs.
- `arc-core`：`CodingAgentSpec` 与 `detect`、`engine` + `adapters`、`skill` 三源注册表、`status`、`market`、`provider`、`paths`、`io`。
  `arc-core`: `CodingAgentSpec` and `detect`, `engine` + `adapters`, `skill` three-source registry, `status`, `market`, `provider`, `paths`, `io`.
- `arc-tui`：模糊搜索、skill browser、provider tab 选择器、skill 安装/卸载向导、项目 skill 编辑器、主题。
  `arc-tui`: fuzzy search, skill browser, provider tab selector, skill install/uninstall wizard, project skill editor, theme.

补充约束 / Additional constraints:

- 终端排版、列表布局和交互模式判定 helper 留在 `arc-cli` / `arc-tui`。
  Terminal layout, list rendering, and interactive-mode detection helpers stay in `arc-cli` / `arc-tui`.
- 文件写入优先复用 `arc-core::io` 的原子写接口。
  File writes should reuse `arc-core::io` atomic write interfaces.
- 不要引入未使用依赖。
  No unused dependencies.

## 路线图备忘 / Roadmap Notes

- P0：`provider` / `market` / `skill` 行为稳定；改动必带测试。
  P0: `provider` / `market` / `skill` behavior stable; changes must include tests.
- P1：market / provider 黑盒与边界测试加强。
  P1: Strengthen market / provider black-box and edge-case tests.
- P2：配置与 provider schema 文档化（持续）。
  P2: Configuration and provider schema documentation (ongoing).

## 合并请求规范 / Merge Request Conventions

- 单次 PR 范围尽量单一。
  Keep each PR focused.
- 说明改了什么、为什么、影响哪些命令或磁盘布局。
  Describe what changed, why, and which commands or disk layouts are affected.
- 兼容性变化须写清迁移或破坏面。
  Breaking changes must document migration or impact.
- 不引入未使用依赖；保持现有 Rust 风格。
  No unused dependencies; follow existing Rust style.
