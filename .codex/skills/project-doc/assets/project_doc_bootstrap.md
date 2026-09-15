# arc-kit 工程文档建库备忘录

> 仅在已授权的建库工作尚未完成时保存为 `docs/.project_doc_bootstrap.md`。替换全部占位符；每完成一个工作单元及交接前更新，最终审计完成后删除。

## 当前范围

- 仓库与文档根：`{{repository_root}}` / `docs/`
- 用户要求的建库范围：{{authorized_scope}}
- 工程正文语言：中文；README 与 CONTRIBUTING 保留中英文入口。
- 总体状态：`{{in_progress_or_blocked}}`
- 当前工作单元：{{current_item}}
- 下一步：{{next_action_with_exact_source_or_target}}
- 最后更新：{{timestamp_with_timezone}}

## 工作区与核实基线

- 当前分支及既有修改概况：{{branch_and_existing_changes}}
- 已核实来源及范围：{{current_files_symbols_and_tests}}
- 本次允许修改的载体：{{docs_readme_anchors_or_instruction_paths}}
- 恢复时需重新检查的来源：{{sources_changed_or_not_yet_verified}}

## 结构决策

- 已确定的分类、文档边界与理由：{{structure_decisions}}
- 现有材料的归属与引用策略：{{existing_material_disposition}}
- 不在本次建库范围内的内容：{{out_of_scope}}

## 文档计划

<!-- 状态使用 planned、in_progress、verified、blocked；计划路径保持代码格式。 -->
| 路径 | 范围与读取时机 | 代码、符号与测试依据 | 状态 | 下一步 |
| --- | --- | --- | --- | --- |
| `{{document_path}}` | {{scope_and_read_when}} | {{source_paths_and_symbols}} | `{{status}}` | {{next_step}} |

## 现有文档同步

| 载体 | 相关章节 | 处置与理由 | 同步情况 |
| --- | --- | --- | --- |
| `README.md` / `README.zh-CN.md` | {{sections}} | {{keep_link_or_update}} | {{sync_status}} |
| `CONTRIBUTING.md` / `CONTRIBUTING.zh-CN.md` | {{sections_or_not_applicable}} | {{disposition}} | {{sync_status}} |
| `{{existing_resource_doc_path}}` | {{sections_or_not_applicable}} | {{disposition_or_not_applicable}} | {{sync_status}} |

## 锚点与路径变更

<!-- 不需要锚点或迁移时写“无”，不保留空计划行。 -->
| 文档与稳定 ID | 手写语义入口 | 旧路径或 ID 的处置 | 状态 |
| --- | --- | --- | --- |
| `{{document_path}}#{{section_id}}` | `{{code_path_and_symbol}}` | {{old_reference_disposition}} | `{{status}}` |

## 接入检查

- `AGENTS.md` 的技能门禁与技能内的文档入口：{{gate_status}}
- README 双语文档分工、工程入口与注释语言：{{readme_contract_status}}
- `.claude/skills` 共享链接与实际技能目录的去重检查：{{shared_skill_entry_status}}
- 其他本次涉及的指令入口：{{other_entries_or_none}}
- 根目录、分类目录和规范条目：{{index_status}}

## 阻塞与交接

- 已完成并可保留的结果：{{verified_results}}
- 待核实契约及继续所需信息：{{uncertainties_or_none}}
- 当前阻塞及恢复条件：{{blockers_or_none}}
- 交接后第一个具体动作：{{exact_next_action}}

## 验证记录

| 范围 | 实际检查或命令 | 结果及未覆盖项 |
| --- | --- | --- |
| {{scope}} | {{validation_actually_performed}} | {{result_and_limits}} |
