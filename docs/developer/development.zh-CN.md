# 开发指南

[English](development.md)

本文覆盖贡献流程、仓库结构和验证门禁。命令语义见 [design.zh-CN.md](design.zh-CN.md)；用户工作流见 [../user/guide.zh-CN.md](../user/guide.zh-CN.md)。

## 环境

- Rust stable toolchain
- 目标平台为 macOS

```bash
git clone https://github.com/duoyuli/arc-kit.git
cd arc-kit
cargo check
cargo test
```

## 必要检查

提交代码前：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

如果改动了 CLI 入口、输出格式或交互语义，还要执行：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

版本号变更、打 `v*` tag 或正式发布前：

```bash
./scripts/regression.sh
```

回归脚本会在隔离的 `ARC_KIT_USER_HOME` 下执行格式检查、构建、clippy、测试和 CLI 黑盒检查。

## 仓库结构

```text
.
├── arc-cli/          # CLI、clap 命令表、用户输出、JSON 结构体
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、detect、paths、io
├── arc-tui/          # 交互 UI；只有这个 crate 依赖 dialoguer
├── built-in/         # 内置 skill 和 market index
├── docs/             # 正式文档
├── scripts/
│   └── regression.sh # 发版前回归
└── Cargo.toml
```

## 模块职责

- `arc-core`：业务逻辑、状态、文件系统操作、provider 应用、market 同步、skill registry、安装引擎、检测和项目解析。
- `arc-cli`：命令定义、命令分发、用户输出和 JSON 响应结构。
- `arc-tui`：交互式终端 UI、选择器、模糊浏览、向导流程和主题。

不要把业务逻辑放进 `arc-cli`。不要把 `dialoguer` 交互放进 `arc-core` 或 `arc-cli`。

## 文档要求

行为类代码变更必须更新相关文档：

- 面向产品能力变化的 `README.md`；
- 用户工作流对应的 `docs/user/guide.md`；
- CLI 语义、JSON 或交互变化对应的 `docs/developer/design.md`；
- 构建、测试、发版或模块职责变化对应的 `docs/developer/development.md`；
- 对应的 `.zh-CN.md` 中文镜像。

代码注释和 CLI 提示使用英文。正式文档维护为英文默认文件加中文镜像文件。

## 贡献规则

- 每次变更保持范围聚焦。
- 行为变更必须带测试。
- 避免无关重构。
- 不引入未使用依赖。
- 持久化写入使用 `arc-core::io` 的原子写 helper。
- 终端布局和交互模式判断应靠近 CLI/TUI 边界。

## 发版规则

- 先确认 `main` push 成功，再推 release tag。
- 单独推送 tag。
- 不要执行 `git push origin main --tags`。

## 路线图备忘

- P0：provider、market 和 skill 行为必须保持稳定；改动需要测试。
- P1：加强 market/provider 黑盒和边界测试。
- P2：持续完善配置和 provider schema 行为文档。
