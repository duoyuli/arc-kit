本文件为 Agent 在此仓库中的最小工作约束。细节规范以 [docs/developer/design.md](docs/developer/design.md) 和 [docs/developer/development.md](docs/developer/development.md) 为准。

## 项目

- `arc-kit` 是一个 Rust CLI，用于管理 coding agent 的 provider、skill、MCP、subagent 和 market。
- 目标平台仅为 macOS。
- Cargo workspace 主要分为：
  - `arc-cli`：CLI、命令定义、用户输出
  - `arc-core`：领域逻辑、状态与文件系统操作
  - `arc-tui`：交互式终端 UI

## 必守规则

- 业务逻辑放在 `arc-core`，不要塞进 `arc-cli`。
- TUI / `dialoguer` 交互只放在 `arc-tui`。
- 所有行为变更必须带测试。
- 有代码变动时，必须同步更新 `README.md` 与 `docs/`。
- 不要混入无关重构；不要引入未使用依赖。
- 代码注释与命令行提示使用英文；文档使用中文。

## CLI 语义

- CLI 只有两类语义：**交互式** 与 **非交互式**。
- 读取类命令必须支持 `--format json`（已登记例外除外）。
- 带向导的写入类命令必须提供非交互参数路径；非交互下不得读 stdin。
- JSON 输出不得混入 ANSI；退出码语义必须写入文档。
- `skill` / `mcp` / `subagent` 这类资源命令，按整组 `list / info / install / uninstall` 判断是否同时支持 for 人和 for Agent。

交互与自动化细则见 [docs/developer/design.md](docs/developer/design.md)。

## 验证

- 提交前至少运行：

```bash
cargo fmt --all
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

- 如果修改了 CLI 命令，还要补：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
```

- 发版前必须执行：

```bash
./scripts/regression.sh
```

完整回归、黑盒矩阵和开发规范见 [docs/developer/development.md](docs/developer/development.md)。

## 发版

- 先确认 `main` 推送成功，再单独打 tag 推送。
- 不要执行 `git push origin main --tags`。

## 风格

- 保持实现与交互简单、直接、可靠。
