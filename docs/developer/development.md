# 开发与贡献

命令与交互设计见 [用户手册](../user/guide.md) 与 [交互与自动化设计](design.md)。仓库里的 agent 支持、项目级路径和内置资源命名，都以当前代码实现为准。

## 贡献方式

- 缺陷：提交 Issue，写清复现步骤、预期/实际行为、系统与已安装的 coding agent。
- 较大功能或重构：先讨论范围再动手。
- 行为变更：须补测试，覆盖改动到的核心路径。

## 开发环境

- Rust：稳定版 toolchain
- 平台：当前以 macOS 为主

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

## 提交前检查

仓库根目录执行：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

若改动 CLI 入口、输出格式或交互语义，须补充黑盒检查：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

## 发版前完整回归

版本号变更、打 `v*` tag 或正式发布前，必须通过：

```bash
./scripts/regression.sh
```

脚本内容是 `cargo fmt --all --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，以及在隔离的 `ARC_KIT_USER_HOME` 下执行 CLI 黑盒。覆盖内容包括：

- `status --format json` 的模块存在性（`project`、`agents`、`catalog`、`actions`）
- `skill install`、`skill uninstall`、`provider use` 在 `--format json` 下的非交互缺参失败
- `skill info` 的结构化 JSON 错误
- `mcp` / `subagent` 命令已移除
- `[mcps]` 等移除后的 `arc.toml` section 会被拒绝

## 仓库结构

```text
.
├── arc-cli/          # CLI、clap、用户输出、format JSON
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、detect
├── arc-tui/          # 交互 UI（仅本 crate 依赖 dialoguer）
├── built-in/         # 内置 skill 与 market 索引
├── docs/             # 官方文档
├── scripts/
│   └── regression.sh # 发版前回归
└── Cargo.toml
```

## 模块职责

- `arc-cli`：`app` 编排、`cli` 命令表、`commands/*`、`format.rs` JSON 结构体。
- `arc-core`：`CodingAgentSpec` 与 `detect`、`engine` + `adapters`、`skill` 三源注册表、`status`、`market`、`provider`、`paths`、`io`。
- `arc-tui`：模糊搜索、skill browser、provider tab 选择器、skill 安装/卸载向导、项目 skill 编辑器、主题。

补充约束：

- 终端排版、列表布局和交互模式判定 helper 留在 `arc-cli` / `arc-tui`。
- 文件写入优先复用 `arc-core::io` 的原子写接口。
- 不要引入未使用依赖。

## 路线图备忘

- P0：`provider` / `market` / `skill` 行为稳定；改动必带测试。
- P1：market / provider 黑盒与边界测试加强。
- P2：配置与 provider schema 文档化（持续）。

## 合并请求规范

- 单次 PR 范围尽量单一。
- 说明改了什么、为什么、影响哪些命令或磁盘布局。
- 兼容性变化须写清迁移或破坏面。
- 不引入未使用依赖；保持现有 Rust 风格。
