本文件为 Agent 在此仓库中工作时提供指导。

## 项目概述

arc-kit 是一个 Rust CLI 工具，用于管理编码智能体的能力——包括 provider 切换、skill/MCP server 管理和 market 发现。仅支持 macOS。

## 构建与开发

```bash
cargo fmt --all          # 格式化
cargo check              # 编译检查
cargo clippy --all-targets -- -D warnings  # lint 检查
cargo test               # 运行所有测试
cargo run -p arc-cli -- status   # 本地运行
cargo install --path arc-cli --force  # 安装二进制
```

提交前必须运行：`cargo fmt --all && cargo check && cargo clippy --all-targets -- -D warnings && cargo test`

如果修改了 CLI 命令，还需要补充黑盒检查：
```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

**发版前**必须完成完整回归（含自动化测试与 CLI 黑盒），标准见 [`docs/developer/development.md`](docs/developer/development.md)。一键执行：

```bash
./scripts/regression.sh
```

## 发版流程

```bash
# 0. 完整回归通过（./scripts/regression.sh）
# 1. 改版本号（Cargo.toml workspace.package.version）
# 2. cargo check 更新 Cargo.lock
# 3. 提交并推 main——必须先确认 main 推送成功
git add -A && git commit -m "升级 vX.Y.Z"
git pull --rebase origin main   # 如有远程变动先 rebase
git push origin main

# 4. main 推成功后，再单独打 tag 推送——只推一次
git tag vX.Y.Z
git push origin vX.Y.Z
```

严禁 `git push origin main --tags` 一起推。如果 main 被 reject 需要 rebase，tag 会指向错误的 commit，force push tag 会导致 release workflow 触发两次。

## Workspace 结构

Cargo workspace 包含三个 crate：

- `arc-cli` — CLI 入口、命令定义、用户输出
- `arc-core` — 核心领域逻辑、状态管理、文件系统操作
- `arc-tui` — 交互式终端 UI 流程

保持 CLI 与核心逻辑分离。业务逻辑放在 arc-core，不要放在 arc-cli。

## 代码规范

- Rust 2024 edition，stable toolchain
- 所有行为变更必须附带测试
- 每个 PR 范围单一，不要混入无关重构
- 不引入未使用的依赖
- 代码中所有注释以及命令行提示都使用英文
- 文档使用中文

## 同步机制

如有代码变动
- 要同步增加对应的单元测试
- 同步修正 README.md 及 ./docs 下的文档内容

## 合并请求规范

- 单次 PR 范围单一，不要混入无关重构
- 说明改了什么、为什么、影响哪些命令或磁盘布局
- 兼容性变化须写清迁移或破坏面
- 不引入未使用的依赖；保持现有 Rust 风格

## 交互式与非交互式

arc-kit 的 CLI 语义只有两类：**交互式**（人在 TTY）与**非交互式**（管道/CI 或 `--format json`）。新功能须同时覆盖二者，详见 `docs/developer/design.md`。

**判定摘要：**
- **交互式**：stdin/stdout 均为 TTY，且未使用 `--format json` → TUI、彩色、Confirm
- **非交互式**：否则 → 纯文本（默认 `text`）或 `--format json`（`--format json` 优先于 TTY）

**核心约束：**
- 读取类须覆盖交互式与非交互式，且须支持 `--format json`（稳定 schema；已登记例外除外，如 `version`）
- 带向导的写入类须提供显式参数，使**非交互式**下一键完成同一语义，不得阻塞于 stdin
- JSON 输出不得混入 ANSI 颜色码
- TUI 组件（dialoguer）的交互调用与主题渲染仅存在于 `arc-tui` crate；为 TUI 准备的数据映射可酌情放在 `arc-core`
- 退出码是 Agent 的首要信号，语义必须写进文档

## 品味偏好

- 项目功能设计参考 Elon Musk 的风格，
- 产品交互设计参考 Steve Paul Jobs 的风格，
- 代码实现追求 Linus Benedict Torvalds 式的简洁、直接与可靠。
