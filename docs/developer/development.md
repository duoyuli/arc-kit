# 开发与贡献

命令与交互设计见 [用户手册](../user/guide.md) 与 [交互与自动化设计](design.md)。

---

## 贡献方式

- **缺陷**：提交 Issue，写清复现步骤、预期/实际行为、系统与已安装的 coding agent。
- **较大功能或重构**：先讨论范围再动手。
- **行为变更**：须补测试，覆盖改动到的核心路径。

## 开发环境

- Rust：稳定版 toolchain  
- 平台：当前以 **macOS** 为主（产品目标平台）

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

## 提交前检查（最小）

仓库根目录执行：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

若改动 CLI 入口，须补充黑盒检查：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

与根目录 `CLAUDE.md` / `AGENTS.md` 中「提交前」要求一致。

## 发版前完整回归

版本号变更、打 `v*` tag 或正式发布前，**必须**通过：

```bash
./scripts/regression.sh
```

脚本内容：`cargo fmt --all --check`、`check`、`clippy -D warnings`、`test`，以及在**隔离 `ARC_KIT_USER_HOME`** 且**工作目录无 `arc.toml`** 下的 CLI 黑盒（避免在 arc-kit 仓库根直接跑 `status` 命中仓库内 `arc.toml`）。

### 回归适用范围

| 场景 | 要求 |
|------|------|
| 日常提交 / PR | 最小检查 |
| 发版 / 打 tag | 完整回归 |
| 仅文档、注释、CI 且不影响构建 | 至少 `fmt --check` 与 `cargo check`（或评审约定） |

### 黑盒矩阵（脚本已覆盖核心项）

手动补充时可在隔离环境下核对：`--help`、`version`、`status`、`status --format json`、`project --help`、`skill list`、`market list`、`provider list`、`completion zsh` 等。

若改动触及 **provider 连通性**、**market 拉取** 或 **JSON 语义**，追加 `provider test` 及相应子命令的 `--format json` 与退出码（见 [交互与自动化设计](design.md)）。

### 退出准则

完整回归通过：`fmt --check`、`check`、`clippy`、`test` 全部成功，且 `./scripts/regression.sh` 退出码 `0`。再打 tag。发版流程（版本号、`main` 与 tag 分步推送）见根目录 `CLAUDE.md`。

---

## 仓库结构（Cargo workspace）

```text
.
├── arc-cli/          # CLI、clap、用户输出、format JSON
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、detect
├── arc-tui/          # 交互 UI（仅本 crate 依赖 dialoguer）
├── built-in/         # 内置 skill 与 market 索引
├── docs/             # 官方文档（分层见 docs/README.md）
├── scripts/
│   └── regression.sh # 发版前回归
└── Cargo.toml
```

### 模块职责摘要

- **arc-cli**：`app` 编排、`cli` 命令表、`commands/*`、`format.rs` JSON 结构体。
- **arc-core**：`CodingAgentSpec` 与 `detect`、`engine` + `adapters`、`skill` 三源注册表、`market`、`provider`、`paths`、`io`。
- **arc-tui**：模糊搜索、skill/provider 向导、主题。

---

## 路线图备忘

- **P0**：`provider` / `market` / `skill` 行为稳定；改动必带测试。
- **P1**：market / provider 黑盒与边界测试加强。
- **P2**：配置与 provider schema 文档化（持续）。

**设计原则**（节选）：CLI / core / TUI 分层；agent 元数据集中在 `CodingAgentSpec`；安装走统一 adapter；只读 JSON 与写入一键路径见 [交互与自动化设计](design.md)。

**不在范围**：Windows 支持等（以根目录 README / 产品声明为准）。

---

## 合并请求规范

- 单次 PR 范围尽量单一。
- 说明改了什么、为什么、影响哪些命令或磁盘布局。
- 兼容性变化须写清迁移或破坏面。
- 不引入未使用依赖；保持现有 Rust 风格。

## 许可证

参与贡献即表示同意以项目当前许可证授权。详见根目录许可证文件。
