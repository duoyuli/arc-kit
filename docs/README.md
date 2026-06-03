# 文档目录 / Documentation Index

官方文档按读者分层，全部位于仓库 `docs/` 下。入口先读根目录 [README.md](../README.md)。

Official documentation is organized by audience, all under `docs/`. Start with the root [README.md](../README.md).

```text
docs/
├── README.md           # 本索引 / This index
├── user/
│   └── guide.md        # 产品使用说明书 / User manual
└── developer/
    ├── design.md       # 交互/非交互设计、JSON 约定、实现对照
    │                   # Interactive/non-interactive design, JSON conventions, implementation
    └── development.md  # 开发规范、测试门禁、仓库结构
                        # Development conventions, test gates, repo structure
```

| 路径 / Path | 说明 / Description |
|------|------|
| [user/guide.md](user/guide.md) | 面向终端用户的一份完整产品使用说明书，覆盖安装、状态、Provider、Market、Skill、项目配置与补全 / Complete user manual covering installation, status, provider, market, skill, project config, and shell completions |
| [developer/design.md](developer/design.md) | 交互式与非交互式、JSON 语义、设计规则、实现对照与缺口 / Interactive vs non-interactive, JSON semantics, design rules, implementation mapping and gaps |
| [developer/development.md](developer/development.md) | 贡献流程、提交与回归、Cargo workspace、内置资源规范 / Contribution workflow, commit & regression, Cargo workspace, built-in resource conventions |
