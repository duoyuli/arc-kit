# 为 arc-kit 贡献

[English](CONTRIBUTING.md)

感谢贡献。本文件是简短贡献入口；更完整的流程和命令语义见开发者文档。

## 开始之前

- 缺陷：提交 issue，写清复现步骤、预期行为、实际行为、macOS 版本和已安装的 coding agent。
- 较大功能或重构：先讨论范围，再开始实现。
- 行为变更：必须添加覆盖受影响核心路径的测试。

## 开发环境

- Rust stable toolchain
- 目标平台为 macOS

```bash
cargo check
cargo test
```

## 必要检查

提交代码前，在仓库根目录执行：

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

## 文档同步

代码变更必须同步更新相关文档：

- 面向产品说明的根目录 `README.md`；
- 用户流程对应的 `docs/user/guide.md`；
- 命令语义、JSON 或交互设计对应的 `docs/developer/design.md`；
- 开发流程或发版门禁对应的 `docs/developer/development.md`；
- 中文镜像对应的 `.zh-CN.md` 文件。

## Pull Request 规范

- 每个 PR 保持范围单一。
- 避免无关重构。
- 说明改了什么、为什么改、影响哪些命令或磁盘布局。
- 破坏性变更需要说明迁移方式或兼容性影响。
- 不引入未使用依赖。

## 更多信息

- [开发指南](docs/developer/development.zh-CN.md)
- [交互与 JSON 设计](docs/developer/design.zh-CN.md)
- [用户手册](docs/user/guide.zh-CN.md)
