# 开发与贡献

命令与交互设计见 [用户手册](../user/guide.md) 与 [交互与自动化设计](design.md)。仓库里的 agent 支持、项目级路径和内置资源命名，都以当前代码实现为准，不以旧文档或审查结论为准。

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

若改动 CLI 入口、输出格式或交互语义，须补充黑盒检查：

```bash
cargo run -p arc-cli -- --help
cargo run -p arc-cli -- status
cargo run -p arc-cli -- status --format json
```

与根目录 `AGENTS.md` 中「提交前」要求一致。

## 发版前完整回归

版本号变更、打 `v*` tag 或正式发布前，**必须**通过：

```bash
./scripts/regression.sh
```

脚本内容是 `cargo fmt --all --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，以及在隔离的 `ARC_KIT_USER_HOME` 和内置清单覆盖下执行的 CLI 黑盒。除空目录 smoke 外，还覆盖：
- `status --format json` 的模块存在性（含 `mcps`、`subagents`、`actions`）
- `skill install`、`skill uninstall`、`provider use` 在 `--format json` 下的非交互缺参失败
- `skill info`、`mcp info`、`subagent info` 的结构化 JSON 错误
- `project apply` 的项目 capability 安装、目标收缩清理，以及 `--allow-global-fallback` 后的 `status` 反映

### 回归适用范围

| 场景 | 要求 |
|------|------|
| 日常提交 / PR | 最小检查 |
| 发版 / 打 tag | 完整回归 |
| 仅文档、注释、CI 且不影响构建 | 至少 `fmt --check` 与 `cargo check`（或评审约定） |

### 黑盒矩阵（脚本已覆盖核心项）

手动补充时可在隔离环境下核对：`--help`、`version`、`status`、`status --format json`、`project --help`、`project apply --help`、`project edit --help`、`skill list`、`mcp list`、`subagent list`、`market list`、`provider list`、`completion zsh`，以及 `project apply` 的 capability 回归场景。

若改动触及 **provider 连通性**、**market 拉取** 或 **JSON 语义**，追加 `provider test` 及相应子命令的 `--format json` 与退出码（见 [交互与自动化设计](design.md)）。

### 退出准则

完整回归通过：`fmt --check`、`check`、`clippy`、`test` 全部成功，且 `./scripts/regression.sh` 退出码 `0`。再打 tag。发版流程（版本号、`main` 与 tag 分步推送）见根目录 `AGENTS.md`。

---

## 仓库结构（Cargo workspace）

```text
.
├── arc-cli/          # CLI、clap、用户输出、format JSON
├── arc-core/         # 领域逻辑、安装引擎、provider、market、skill、mcp、subagent、detect
├── arc-tui/          # 交互 UI（仅本 crate 依赖 dialoguer）
├── built-in/         # 内置只读资源：skill/、market/、mcp/
├── docs/             # 官方文档（分层见 docs/README.md）
├── scripts/
│   └── regression.sh # 发版前回归
└── Cargo.toml
```

### 模块职责摘要

- **arc-cli**：`app` 编排、`cli` 命令表、`commands/*`、`format.rs` JSON 结构体。
- **arc-core**：`CodingAgentSpec` 与 `detect`、`engine` + `adapters`、`skill` 三源注册表、`capability`（mcp / subagent canonical source、tracking、落地）、`market`、`provider`、`paths`、`io`。
- **arc-tui**：模糊搜索、skill/provider 向导、主题。

---

## 内置 MCP 维护规范

内置 MCP 预设文件位于 `built-in/mcp/index.toml`。这里存放的是**只读 preset 来源**：`arc mcp list` / `arc mcp info` 会读取它，`arc mcp install <name>` 会以它为模板落到用户 registry。新增或调整内置 MCP 时，按以下约束执行：

### 字段规范

- 仅使用 `McpDefinition` 已支持字段：`name`、`targets`、`transport`、`command`、`args`、`env`、`url`、`headers`、`description`。
- 不要写未落地到 schema 的字段，例如 `doc_url`、`transport_type`、`http`。文档链接应写入用户手册或本节说明，不写入 preset schema。
- transport 只允许：
  - `stdio`
  - `sse`
  - `streamable_http`
- 其中：
  - `stdio` 必须有 `command`，不能有 `url`
  - `sse` / `streamable_http` 必须有 `url`，不能有 `command`

### 命名与安全

- `name` 必须匹配 `^[a-z0-9][a-z0-9-_]{0,63}$`，优先使用社区常见名称，避免过度缩写。
- `description` 保持英文，简短说明“是什么 + 怎么运行”，不要写营销文案。
- 涉及认证的 `env` / `headers` 必须使用环境变量占位符，例如 `${API_KEY}`、`Bearer ${API_KEY}`；禁止提交明文 secret。
- 若上游文档使用 `http` 之类的口语化 transport，落地到 arc-kit 时必须映射为当前 schema 中的 `streamable_http`，并在文档里写清映射关系。

### 文档同步

- 新增内置 MCP 时，必须同步更新 [docs/user/guide.md](../user/guide.md) 的 `mcp` 章节：
  - 补充预设列表
  - 补充至少一个安装示例
  - 若有鉴权或特殊前置条件，写清环境变量名与用法
- 若根目录 README 的能力描述或 FAQ 已受影响，也要同步更新 [README.md](../../README.md)。
- 原则是：用户在文档里能直接看懂“这个预设是什么、怎么装、需要什么环境变量、如何覆盖默认值”。

### 测试要求

- 至少保证内置 preset 解析测试覆盖新增项，避免发布后出现 TOML/schema 不兼容。
- 若新增项带来新的用户手册承诺或 JSON 可见行为，补 CLI 黑盒测试。
- 提交前执行最小检查；发版前走完整回归。

---

## 路线图备忘

- **P0**：`provider` / `market` / `skill` / `mcp` / `subagent` 行为稳定；改动必带测试。
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

参与贡献即表示同意以项目当前许可证声明为准。
