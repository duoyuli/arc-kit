# 参与贡献 arc-kit

感谢你的贡献！详细流程、发版门禁与仓库结构见 **[docs/developer/development.md](docs/developer/development.md)**；命令语义与 JSON 约束见 **[docs/developer/design.md](docs/developer/design.md)**。

---

## 贡献方式

- **缺陷**：提交 Issue，写清复现步骤、预期/实际行为、系统与已安装的 coding agent。
- **较大功能或重构**：先讨论范围再动手。
- **行为变更**：须补测试，覆盖改动到的核心路径。

## 开发环境

- Rust：稳定版 toolchain
- 平台：当前以 **macOS** 为主（产品目标平台）

## 提交前检查

仓库根目录**必须**执行：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

若改动 CLI 命令或参数，**还须**补充黑盒检查：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

## 代码规范

- Rust 2024 edition，stable toolchain
- 代码中所有注释以及命令行提示都使用**英文**
- 文档使用**中文**
- 不引入未使用的依赖

## 同步机制

如有代码变动：
- 同步增加对应的单元测试
- 同步修正 `README.md` 及 `./docs` 下的文档内容

## 合并请求规范

- 单次 PR 范围单一，不要混入无关重构
- 说明改了什么、为什么、影响哪些命令或磁盘布局
- 兼容性变化须写清迁移或破坏面
- 保持现有 Rust 风格

## 许可证

参与贡献即表示你同意以项目当前许可证声明为准。
