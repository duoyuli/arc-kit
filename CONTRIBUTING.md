# 参与贡献 arc-kit / Contributing to arc-kit

感谢你的贡献！详细流程、发版门禁与仓库结构见 **[docs/developer/development.md](docs/developer/development.md)**；命令语义与 JSON 约束见 **[docs/developer/design.md](docs/developer/design.md)**。

Thank you for contributing! Full workflow, release gates, and repo structure are in **[docs/developer/development.md](docs/developer/development.md)**; command semantics and JSON conventions are in **[docs/developer/design.md](docs/developer/design.md)**.

---

## 贡献方式 / How to Contribute

- **缺陷 / Bugs**：提交 Issue，写清复现步骤、预期/实际行为、系统与已安装的 coding agent。
  Submit an Issue with reproduction steps, expected/actual behavior, OS version, and installed coding agents.
- **较大功能或重构 / Major features or refactors**：先讨论范围再动手。
  Discuss the scope before starting.
- **行为变更 / Behavioral changes**：须补测试，覆盖改动到的核心路径。
  Must include tests covering the affected core paths.

## 开发环境 / Development Environment

- Rust：稳定版 toolchain / Stable toolchain
- 平台：当前以 **macOS** 为主（产品目标平台）/ Platform: **macOS** primary (product target)

## 提交前检查 / Pre-commit Checks

仓库根目录**必须**执行 / **Must** run at repo root:

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

若改动 CLI 命令或参数，**还须**补充黑盒检查 / If CLI commands or parameters changed, **also** run:

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

## 代码规范 / Code Conventions

- Rust 2024 edition，stable toolchain
- 代码中所有注释以及命令行提示都使用**英文** / All code comments and CLI prompts in **English**
- 文档使用**中文+英文** / Documentation in **Chinese + English**
- 不引入未使用的依赖 / No unused dependencies

## 同步机制 / Sync Requirements

如有代码变动 / When making code changes:
- 同步增加对应的单元测试 / Add corresponding unit tests
- 同步修正 `README.md` 及 `./docs` 下的文档内容 / Update `README.md` and docs under `./docs`

## 合并请求规范 / Pull Request Guidelines

- 单次 PR 范围单一，不要混入无关重构 / Keep PRs focused; no unrelated refactors
- 说明改了什么、为什么、影响哪些命令或磁盘布局 / Describe what changed, why, and which commands or disk layouts are affected
- 兼容性变化须写清迁移或破坏面 / Breaking changes must document migration or impact
- 保持现有 Rust 风格 / Follow existing Rust style

## 许可证 / License

参与贡献即表示你同意以项目当前许可证声明为准。
By contributing, you agree to the project's current license terms.
