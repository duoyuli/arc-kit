# arc-kit 仓库导航

此表将任务路由到现有实现和测试，用于 `docs/index.md` 尚未建立时的入口，以及文档同步时的代码定位。表内仓库路径均从仓库根开始；它们是查证入口，行为和版本仍需回读当前文件。

## 文档现状与所有权

本次重写时，仓库已在 `AGENTS.md` 接入技能门禁，但没有 `docs/index.md`、建库备忘录或源码中的 `@project-doc` 锚点。每次任务重新检查实际文件；不要把这个状态固化为今后的前提。

| 现有载体 | 负责内容 | 读取时机 |
| --- | --- | --- |
| `AGENTS.md` | 技能门禁、中文代码注释、Conventional Commits、当前分支约束 | 开始仓库任务时 |
| `README.md`、`README.zh-CN.md` | 产品能力、用户指南、CLI/JSON/交互契约、开发与发版流程 | 涉及对应功能或开发流程时，两份对应章节交叉核实 |
| `CONTRIBUTING.md`、`CONTRIBUTING.zh-CN.md` | 简短贡献说明及 README 导航 | 改变贡献要求、必要检查或文档入口时 |
| `built-in/` | 内置 market 索引和 skill 资源的来源目录 | 改变资源嵌入、扫描或分发内容时核实实际文件 |
| `.codex/skills/project-doc/`，由 `.claude/skills` 共享 | 仓库内的文档协作方法；两个 Agent 入口对应同一份文件 | 改变文档流程、入口或本技能时，按实际路径去重 |

根 `arc.toml` 被 `.gitignore` 排除，是可选的本地试用配置。描述项目配置契约时读取解析器和测试，不要求仓库必须提供这个文件，也不把个人配置纳入正式文档。

当前 `.gitignore` 未排除 `.codex/` 和 `.claude/`，两处技能入口可正常纳入版本控制，是否已跟踪需查看 `git ls-files`。`.claude/skills` 是指向 `../.codex/skills` 的符号链接，共享实际技能目录；不要把它当作第二份副本。

`built-in/skill/` 当前没有技能正文，使用 `.gitkeep` 保留编译期嵌入需要的目录；内置来源机制仍在 `arc-core/src/skill/builtin.rs`。专用测试夹具位于 `arc-core/tests/fixtures/builtin_skills/`，仅供测试嵌入，不随产品分发。核实内置能力时分别检查机制、实际资源与测试，不能因机制存在就宣称产品附带某篇技能。

## 按模块定位

| 任务与读取时机 | 先读实现 | 核实测试与相关说明 |
| --- | --- | --- |
| 工作区、模块边界、构建依赖 | `Cargo.toml`、`arc-core/Cargo.toml`、`arc-cli/Cargo.toml`、`arc-tui/Cargo.toml`；`arc-core/src/lib.rs`；`arc-cli/src/main.rs`、`arc-cli/src/lib.rs`、`arc-cli/src/app.rs`；`arc-tui/src/lib.rs` | README 开发指南；Rust 版本从 workspace 读取 |
| 共享模型、来源优先级、错误与退出码 | `arc-core/src/models.rs`、`arc-core/src/error.rs`；`arc-cli/src/app.rs`、`arc-cli/src/format.rs` | `arc-core/tests/models_tests.rs`；`arc-cli/tests/cli_smoke.rs`、`arc-cli/tests/provider_schema.rs`；核实 `SkillOrigin::priority`、`ArcError` 和调用方 |
| 命令参数、运行模式、JSON、退出码 | `arc-cli/src/cli.rs`、`arc-cli/src/app.rs`、`arc-cli/src/commands/common.rs`、`arc-cli/src/format.rs`、`arc-cli/src/commands/` | `arc-cli/tests/cli_smoke.rs`、`arc-cli/tests/project_config.rs`、`arc-cli/tests/project_tracking.rs`、`arc-cli/tests/provider_schema.rs`；README 交互与自动化设计 |
| Provider profile、原生配置、登录态、连通性测试 | `arc-core/src/provider/mod.rs`、`arc-core/src/provider/claude.rs`、`arc-core/src/provider/codex.rs`、`arc-core/src/provider/test.rs`、`arc-core/src/provider/seed/`；`arc-cli/src/commands/provider.rs` | `arc-core/tests/provider_tests.rs`、`arc-core/tests/provider_schema_tests.rs`；`arc-cli/tests/provider_schema.rs`、`arc-cli/tests/provider_codex_config.rs`；README Provider |
| Agent 能力、检测、全局与项目路径 | `arc-core/src/agent/mod.rs`、`arc-core/src/detect.rs`、`arc-core/src/paths.rs`、`arc-core/src/adapters/` | `arc-core/tests/detect_tests.rs`、`arc-core/tests/paths_tests.rs`、`arc-core/tests/install_engine_tests.rs`；README Skills 路径表 |
| Skill 来源优先级、目录扫描、内置资源 | `arc-core/src/models.rs`、`arc-core/src/skill/registry.rs`、`arc-core/src/skill/local.rs`、`arc-core/src/skill/builtin.rs`、`arc-core/src/skill/merge.rs`；`built-in/skill/` | `arc-core/tests/models_tests.rs`、`arc-core/tests/skill_registry_tests.rs`、`arc-core/tests/skill_local_tests.rs`、`arc-core/tests/skill_builtin_tests.rs`、`arc-core/tests/fixtures/builtin_skills/`；内置扫描和解包另有模块内测试；README Skill 来源 |
| 安装归属、台账、替换、删除、恢复 | `arc-core/src/skill/install/mod.rs`、`arc-core/src/skill/install/state.rs`、`arc-core/src/skill/install/paths.rs`、`arc-core/src/skill/install/transaction.rs`、`arc-core/src/skill/install/global.rs`；`arc-core/src/skill/tracking.rs`、`arc-core/src/engine.rs`、`arc-core/src/adapters/` | `arc-core/tests/installation_state_tests.rs`、`arc-core/tests/tracking_recovery_tests.rs`、`arc-core/tests/install_engine_tests.rs`，以及 `arc-core/src/skill/install/` 内的单元测试；README Markets 台账说明 |
| 项目声明解析、发现、合并配置 | `arc-core/src/project/file.rs`、`arc-core/src/project/discover.rs`、`arc-core/src/project/resolve.rs`；`arc-cli/src/commands/edit.rs`、`arc-cli/src/commands/arc_toml_wizard.rs`；`arc-tui/src/project.rs` | `arc-core/tests/project_apply_tests.rs`；`arc-cli/tests/project_config.rs`；README 项目配置 |
| 项目 apply/clean、agent 选择、接管、预演与清理 | `arc-core/src/project/apply.rs`、`arc-core/src/project/skills.rs`；`arc-cli/src/commands/apply.rs`；共享 `arc-core/src/skill/install/` | `arc-core/tests/project_tracking_tests.rs`；`arc-cli/tests/project_tracking.rs`、`arc-cli/tests/project_config.rs`；README 项目配置 |
| Market URL、索引、扫描、catalog、全局技能刷新 | `arc-core/src/market/`、`arc-core/src/git.rs`、`arc-core/src/skill/sync.rs`；`arc-cli/src/commands/market.rs`；`built-in/market/index.toml` | `arc-core/tests/git_url_tests.rs`、`arc-core/tests/git_tests.rs`、`arc-core/tests/index_tests.rs`、`arc-core/tests/scanner_tests.rs`、`arc-core/tests/catalog_tests.rs`、`arc-core/tests/sources_tests.rs`、`arc-core/tests/bootstrap_tests.rs`、`arc-core/tests/document_tests.rs`、`arc-core/tests/skill_sync_tests.rs`；README Markets |
| 只读状态、逐目标动作、跟踪错误 | `arc-core/src/status.rs`、`arc-core/src/status/`、`arc-core/src/skill/install/mod.rs`；`arc-cli/src/commands/status.rs`、`arc-cli/src/format.rs` | `arc-cli/tests/cli_smoke.rs`、`arc-cli/tests/project_tracking.rs`与相关模块内单元测试；README 状态检查与 JSON |
| 显示名、provider 文本布局、TUI、向导与终端布局 | `arc-cli/src/display.rs`、`arc-cli/src/commands/provider_lines.rs`、`arc-cli/src/commands/`；`arc-tui/src/` | `arc-cli/src/commands/provider_lines.rs` 及 TUI 模块内测试；README 交互与 UI 边界 |
| 原子写入、备份、本地状态目录 | `arc-core/src/io.rs`、`arc-core/src/backup.rs`、`arc-core/src/paths.rs`，再读调用方 | `arc-core/tests/io_tests.rs`、`arc-core/tests/backup_tests.rs`、`arc-core/tests/paths_tests.rs`；安装恢复另读 `arc-core/src/skill/install/` |
| 版本、macOS 构建、Homebrew 与发布 | `Cargo.toml`、`.github/workflows/release.yml`、`Formula/arc-kit.rb` | README 与 CONTRIBUTING 的开发、检查和发版要求 |

表内每个路径均可直接从仓库根解析，不继承相邻单元的目录。符号或文件已移动时先搜索当前所有者，再更新导航。目录条目负责路由整个子系统，不要求为每个局部实现文件另建文档。

## 跨模块核实重点

以下约束用于定位文档影响，不替代实际代码阅读。

- **职责边界**：领域规则、文件系统和状态操作属于 `arc-core`；命令表、分发与输出属于 `arc-cli`；交互 UI 属于 `arc-tui`。当前只有 `arc-tui` 依赖 `dialoguer`，`arc-core` 不应向 stdout 输出。
- **支持范围**：当前完整资源家族是 skill，另有 provider、market 和 project 命令。`arc.toml` 接受 `version`、`provider`、`markets`、`skills`；已移除的 MCP/subagent 命令和配置段不能作为现有能力写回文档。
- **来源与安装分离**：同名 skill 来源为 local 优先于 market、market 优先于 built-in；读取 catalog、解析来源和物化内置缓存是不同操作。全局与项目目标由 agent 能力表决定，OpenClaw 使用复制且不支持项目 skill。
- **安装归属**：共享安装服务记录 scope、项目根和实际 target path，并验证链接或副本归属。同一 skill 可以安装在全局和多个项目，不能按 skill 名称把记录合并成一个。目标路径指安装条目本身，不是软链接解析后的源目录。
- **指纹与历史丢失**：新指纹使用带字段长度的 SHA-256；旧副本迁移还需与当前来源核对，无法证明归属时保留为未解决记录。操作日志写入前先持久化台账；台账缺失或损坏且存在日志时，不猜测提交状态，也不自动回滚目标。
- **协调与恢复**：核对 `project apply` 的声明对齐、`project clean` 的保留声明、`--agent`/`--all-agents` 的范围，以及 `--adopt-existing` 的显式接管条件。预检查阻止新的计划不代表没有先恢复历史操作；逐目标提交也不等于整条命令整体回滚。执行中失败、清理备份失败和台账丢失的处理分别看事务实现与测试。
- **只读边界**：`status` 和项目 `--dry-run` 走只读路径；预演不会迁移台账、恢复操作或物化缓存，未解决项不能写成预演成功。
- **全局与项目联动**：`skill` 命令与 `market update` 的安装维护限定全局范围；共享来源变化仍可能被项目软链接读到。描述影响时同时检查来源变化和目标/台账变化。
- **自动化输出**：`--format json` 优先于 TTY；交互要求 stdin、stdout 都是 TTY。核对退出码、JSON `ok` 和逐目标结果，不用单一字段概括所有命令。`version`、裸 `arc`、`completion` 是 README 已列出的 JSON 例外。

## 版本边界

这些版本属于不同载体，修改时分别核对写入、解析和兼容策略，不把它们合并成一个 schema 版本：

| 载体 | 当前值 | 核实入口 |
| --- | --- | --- |
| CLI JSON | 字符串 `"6"` | `arc-cli/src/format.rs` 的 `SCHEMA_VERSION` 与输出类型 |
| 安装台账 | 数字 `2` | `arc-core/src/skill/install/state.rs` 的 `InstallLedger::default`、`parse_ledger` |
| 安装操作日志 Journal | 数字 `1` | `arc-core/src/skill/install/transaction.rs` 的 Journal 写入，`arc-core/src/skill/install/state.rs` 的 `parse_journals` 校验 |
| `arc.toml` | `version` 默认值为数字 `1` | `arc-core/src/project/file.rs` 的 `ProjectConfig`、`default_version` 与解析测试；默认值不等于版本拒绝策略 |

## 验证入口

纯文档变更检查事实来源、双语对应章节、路径和 diff；不因修改 Markdown 就运行安装、provider 切换或发布命令。

Rust 代码变更的现有检查入口为：

```sh
cargo fmt --all --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

需要修复格式时仅处理本次代码变更，避免批量改写既有工作。相关契约测试可先单独执行，例如项目安装协调对应：

```sh
cargo test -p arc-core --test project_tracking_tests
cargo test -p arc-cli --test project_tracking
```

CLI 入口、输出或交互语义变更还需验证 `--help`、`status`、`status --format json`。优先复用 `arc-cli/tests/` 的隔离 fixture；手动执行时使用临时目录设置 `ARC_KIT_USER_HOME`，并清除可能覆盖它的 `ARC_KIT_HOME`。核心测试使用 `ArcPaths::with_user_home` 或 `with_arc_home`。临时项目、agent 假可执行文件和 market 来源按测试需要构造，不依赖开发者的真实安装或凭据。

查看源文件只能说明已核实实现；测试只有实际运行通过后才报告通过。发布流程的文档同步不授权执行打 tag、push 或 release。
