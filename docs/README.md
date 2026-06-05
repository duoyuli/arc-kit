# Documentation

[中文](README.zh-CN.md)

The official documentation is split by audience. The root [README](../README.md) is the product entry point; this directory holds the longer manuals and design references.

## Structure

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

## Documents

| Document | Audience | Purpose |
| --- | --- | --- |
| [user/guide.md](user/guide.md) | users and automation authors | installation, commands, workflows, paths, and JSON usage |
| [developer/design.md](developer/design.md) | maintainers and contributors | interaction modes, JSON contracts, command semantics, and design constraints |
| [developer/development.md](developer/development.md) | contributors | repository structure, build/test gates, release checks, and module ownership |

Chinese mirrors use the same filename with `.zh-CN.md`.
