# 文档

[English](README.md)

正式文档按读者拆分。根目录 [README](../README.zh-CN.md) 是产品入口；本目录放更完整的手册与设计参考。

## 结构

```text
docs/
├── README.md
├── README.zh-CN.md
├── user/
│   ├── guide.md
│   └── guide.zh-CN.md
└── developer/
    ├── design.md
    ├── design.zh-CN.md
    ├── development.md
    └── development.zh-CN.md
```

## 文档

| 文档 | 读者 | 用途 |
| --- | --- | --- |
| [user/guide.zh-CN.md](user/guide.zh-CN.md) | 用户和自动化脚本作者 | 安装、命令、工作流、路径与 JSON 用法 |
| [developer/design.zh-CN.md](developer/design.zh-CN.md) | 维护者和贡献者 | 交互模式、JSON 契约、命令语义和设计约束 |
| [developer/development.zh-CN.md](developer/development.zh-CN.md) | 贡献者 | 仓库结构、构建/测试门禁、发版检查和模块职责 |

英文默认文件使用相同文件名，不带 `.zh-CN` 后缀。
