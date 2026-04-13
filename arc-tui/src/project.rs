use std::collections::HashSet;
use std::io;

use arc_core::mcp_registry::{McpCatalogEntry, McpEntryOrigin};
use arc_core::models::SkillEntry;
use arc_core::subagent_registry::{SubagentCatalogEntry, SubagentEntryOrigin};
use console::{Alignment, Key, Term, measure_text_width, pad_str, style, truncate_str};

use crate::agent::agent_display_name;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectRequirementsSelection {
    pub skills: Vec<String>,
    pub mcps: Vec<String>,
    pub subagents: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RequirementKind {
    Skill,
    Mcp,
    Subagent,
}

impl RequirementKind {
    fn label(self) -> &'static str {
        match self {
            Self::Skill => "Skill",
            Self::Mcp => "MCP",
            Self::Subagent => "Subagent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectRequirementsTab {
    All,
    Skills,
    Mcps,
    Subagents,
}

impl ProjectRequirementsTab {
    const ALL: [Self; 4] = [Self::All, Self::Skills, Self::Mcps, Self::Subagents];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Skills => "Skills",
            Self::Mcps => "MCPs",
            Self::Subagents => "Subagents",
        }
    }

    fn matches(self, kind: RequirementKind) -> bool {
        match self {
            Self::All => true,
            Self::Skills => kind == RequirementKind::Skill,
            Self::Mcps => kind == RequirementKind::Mcp,
            Self::Subagents => kind == RequirementKind::Subagent,
        }
    }
}

#[derive(Debug, Clone)]
struct ProjectRequirementItem {
    name: String,
    kind: RequirementKind,
    origin: String,
    targets: Option<String>,
    description: String,
    search_corpus: String,
    default_selected: bool,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    name_width: usize,
    kind_width: usize,
    origin_width: usize,
}

struct CursorGuard<'a> {
    term: &'a Term,
}

impl Drop for CursorGuard<'_> {
    fn drop(&mut self) {
        let _ = self.term.show_cursor();
    }
}

pub fn run_project_requirements_editor(
    skills: &[SkillEntry],
    mcps: &[McpCatalogEntry],
    subagents: &[SubagentCatalogEntry],
) -> io::Result<Option<ProjectRequirementsSelection>> {
    run_project_requirements_editor_with_defaults(
        skills,
        mcps,
        subagents,
        &ProjectRequirementsSelection::default(),
    )
}

pub fn run_project_requirements_editor_with_defaults(
    skills: &[SkillEntry],
    mcps: &[McpCatalogEntry],
    subagents: &[SubagentCatalogEntry],
    defaults: &ProjectRequirementsSelection,
) -> io::Result<Option<ProjectRequirementsSelection>> {
    let items = build_items(skills, mcps, subagents, defaults);
    let layout = measure_layout(&items);
    let tab_indexes = build_tab_indexes(&items);
    let mut checked: Vec<bool> = items.iter().map(|item| item.default_selected).collect();
    let mut rows: Vec<usize> = tab_indexes
        .iter()
        .map(|indexes| default_row(indexes, &checked))
        .collect();
    let mut scrolls = vec![0usize; ProjectRequirementsTab::ALL.len()];
    let mut active_tab = 0usize;
    let mut prev_drawn = 0usize;
    let mut search = String::new();
    let term = Term::stderr();

    term.hide_cursor()?;
    let _cursor_guard = CursorGuard { term: &term };

    loop {
        let (term_rows, cols) = term.size();
        let visible_rows = (term_rows as usize).saturating_sub(5).clamp(1, 14);
        let max_line_width = (cols as usize).saturating_sub(1).max(1);

        if prev_drawn > 0 {
            clear_drawn_block(&term, prev_drawn)?;
        }

        let current_tab = ProjectRequirementsTab::ALL[active_tab];
        let filtered = filtered_indexes(&items, &tab_indexes[active_tab], &search);

        if filtered.is_empty() {
            rows[active_tab] = 0;
            scrolls[active_tab] = 0;
        } else {
            rows[active_tab] = rows[active_tab].min(filtered.len() - 1);
            let mut scroll = scrolls[active_tab];
            if rows[active_tab] < scroll {
                scroll = rows[active_tab];
            } else if rows[active_tab] >= scroll + visible_rows {
                scroll = rows[active_tab] + 1 - visible_rows;
            }
            scrolls[active_tab] = scroll;
        }

        write_clamped_line(
            &term,
            format!(
                "  {}  {}",
                style("Project Requirements").bold(),
                style(format!(
                    "{} selected",
                    checked.iter().filter(|&&v| v).count()
                ))
                .dim()
            ),
            max_line_width,
        )?;
        write_clamped_line(&term, render_search_line(&search), max_line_width)?;
        write_clamped_line(
            &term,
            render_tab_line(&items, &checked, active_tab),
            max_line_width,
        )?;

        let shown = if filtered.is_empty() {
            write_clamped_line(
                &term,
                format!(
                    "  {}",
                    style(format!(
                        "No matches in {}. Backspace to clear filter or switch tabs.",
                        current_tab.label()
                    ))
                    .dim()
                ),
                max_line_width,
            )?;
            1
        } else {
            let scroll = scrolls[active_tab];
            let mut shown = 0usize;
            for (pos, &item_idx) in filtered.iter().enumerate().skip(scroll).take(visible_rows) {
                shown += 1;
                write_clamped_line(
                    &term,
                    render_item_line(
                        &items[item_idx],
                        layout,
                        checked[item_idx],
                        pos == rows[active_tab],
                    ),
                    max_line_width,
                )?;
            }
            shown
        };

        write_clamped_line(
            &term,
            render_hint_line(
                &items,
                &checked,
                current_tab,
                tab_indexes[active_tab].len(),
                filtered.len(),
            ),
            max_line_width,
        )?;

        prev_drawn = shown + 4;
        term.flush()?;

        match term.read_key()? {
            Key::Escape => {
                clear_drawn_block(&term, prev_drawn)?;
                term.show_cursor()?;
                return Ok(None);
            }
            Key::Enter => {
                clear_drawn_block(&term, prev_drawn)?;
                term.show_cursor()?;
                return Ok(Some(collect_selection(&items, &checked)));
            }
            Key::ArrowLeft | Key::BackTab => {
                active_tab = if active_tab == 0 {
                    ProjectRequirementsTab::ALL.len() - 1
                } else {
                    active_tab - 1
                };
            }
            Key::ArrowRight | Key::Tab => {
                active_tab = (active_tab + 1) % ProjectRequirementsTab::ALL.len();
            }
            Key::ArrowUp => {
                if !filtered.is_empty() {
                    rows[active_tab] = if rows[active_tab] == 0 {
                        filtered.len() - 1
                    } else {
                        rows[active_tab] - 1
                    };
                }
            }
            Key::ArrowDown => {
                if !filtered.is_empty() {
                    rows[active_tab] = (rows[active_tab] + 1) % filtered.len();
                }
            }
            Key::Char(' ') => {
                if let Some(&item_idx) = filtered.get(rows[active_tab]) {
                    checked[item_idx] = !checked[item_idx];
                }
            }
            Key::Backspace => {
                search.pop();
                reset_rows_to_matches(&mut rows, &tab_indexes, &items, &checked, &search);
                scrolls.fill(0);
            }
            Key::Char(ch) if !ch.is_ascii_control() => {
                search.push(ch);
                reset_rows_to_matches(&mut rows, &tab_indexes, &items, &checked, &search);
                scrolls.fill(0);
            }
            _ => {}
        }
    }
}

fn build_items(
    skills: &[SkillEntry],
    mcps: &[McpCatalogEntry],
    subagents: &[SubagentCatalogEntry],
    defaults: &ProjectRequirementsSelection,
) -> Vec<ProjectRequirementItem> {
    let default_skills: HashSet<&str> = defaults.skills.iter().map(String::as_str).collect();
    let default_mcps: HashSet<&str> = defaults.mcps.iter().map(String::as_str).collect();
    let default_subagents: HashSet<&str> = defaults.subagents.iter().map(String::as_str).collect();
    let known_skill_names: HashSet<&str> = skills.iter().map(|entry| entry.name.as_str()).collect();
    let known_mcp_names: HashSet<&str> = mcps
        .iter()
        .map(|entry| entry.definition.name.as_str())
        .collect();
    let known_subagent_names: HashSet<&str> = subagents
        .iter()
        .map(|entry| entry.definition.name.as_str())
        .collect();

    let mut items = Vec::new();

    for entry in skills {
        items.push(ProjectRequirementItem {
            name: entry.name.clone(),
            kind: RequirementKind::Skill,
            origin: entry.origin_display(),
            targets: None,
            description: entry.summary.clone(),
            search_corpus: skill_search_corpus(entry),
            default_selected: default_skills.contains(entry.name.as_str()),
        });
    }
    for name in defaults
        .skills
        .iter()
        .filter(|name| !known_skill_names.contains(name.as_str()))
    {
        items.push(missing_item(name, RequirementKind::Skill));
    }

    for entry in mcps {
        items.push(ProjectRequirementItem {
            name: entry.definition.name.clone(),
            kind: RequirementKind::Mcp,
            origin: mcp_origin_label(&entry.origin).to_string(),
            targets: capability_targets_label(entry.definition.targets.as_ref()),
            description: entry.definition.description.clone().unwrap_or_default(),
            search_corpus: mcp_search_corpus(entry),
            default_selected: default_mcps.contains(entry.definition.name.as_str()),
        });
    }
    for name in defaults
        .mcps
        .iter()
        .filter(|name| !known_mcp_names.contains(name.as_str()))
    {
        items.push(missing_item(name, RequirementKind::Mcp));
    }

    for entry in subagents {
        items.push(ProjectRequirementItem {
            name: entry.definition.name.clone(),
            kind: RequirementKind::Subagent,
            origin: subagent_origin_label(&entry.origin).to_string(),
            targets: capability_targets_label(entry.definition.targets.as_ref()),
            description: entry.definition.description.clone().unwrap_or_default(),
            search_corpus: subagent_search_corpus(entry),
            default_selected: default_subagents.contains(entry.definition.name.as_str()),
        });
    }
    for name in defaults
        .subagents
        .iter()
        .filter(|name| !known_subagent_names.contains(name.as_str()))
    {
        items.push(missing_item(name, RequirementKind::Subagent));
    }

    items.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.kind.cmp(&b.kind))
            .then(a.origin.cmp(&b.origin))
    });
    items
}

fn missing_item(name: &str, kind: RequirementKind) -> ProjectRequirementItem {
    let origin = "missing from catalog".to_string();
    ProjectRequirementItem {
        name: name.to_string(),
        kind,
        origin: origin.clone(),
        targets: None,
        description: String::new(),
        search_corpus: format!("{name} {} {origin}", kind.label()),
        default_selected: true,
    }
}

fn skill_search_corpus(entry: &SkillEntry) -> String {
    let mut out = format!(
        "{} {} {}",
        entry.name,
        RequirementKind::Skill.label(),
        entry.origin_display()
    );
    if !entry.summary.is_empty() {
        out.push(' ');
        out.push_str(&entry.summary);
    }
    out
}

fn mcp_search_corpus(entry: &McpCatalogEntry) -> String {
    let mut out = format!(
        "{} {} {}",
        entry.definition.name,
        RequirementKind::Mcp.label(),
        mcp_origin_label(&entry.origin)
    );
    if let Some(targets) = capability_targets_label(entry.definition.targets.as_ref()) {
        out.push(' ');
        out.push_str(&targets);
    }
    if let Some(description) = &entry.definition.description {
        out.push(' ');
        out.push_str(description);
    }
    out
}

fn subagent_search_corpus(entry: &SubagentCatalogEntry) -> String {
    let mut out = format!(
        "{} {} {} {}",
        entry.definition.name,
        RequirementKind::Subagent.label(),
        subagent_origin_label(&entry.origin),
        entry.definition.prompt_file
    );
    if let Some(targets) = capability_targets_label(entry.definition.targets.as_ref()) {
        out.push(' ');
        out.push_str(&targets);
    }
    if let Some(description) = &entry.definition.description {
        out.push(' ');
        out.push_str(description);
    }
    out
}

fn capability_targets_label(targets: Option<&Vec<String>>) -> Option<String> {
    targets.map(|targets| {
        targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

fn mcp_origin_label(origin: &McpEntryOrigin) -> &'static str {
    match origin {
        McpEntryOrigin::Builtin => "built-in",
        McpEntryOrigin::User => "user",
    }
}

fn subagent_origin_label(origin: &SubagentEntryOrigin) -> &'static str {
    match origin {
        SubagentEntryOrigin::Builtin => "built-in",
        SubagentEntryOrigin::User => "user",
    }
}

fn measure_layout(items: &[ProjectRequirementItem]) -> Layout {
    Layout {
        name_width: items
            .iter()
            .map(|item| measure_text_width(&item.name))
            .max()
            .unwrap_or(0),
        kind_width: items
            .iter()
            .map(|item| measure_text_width(item.kind.label()))
            .max()
            .unwrap_or(0),
        origin_width: items
            .iter()
            .map(|item| measure_text_width(&item.origin))
            .max()
            .unwrap_or(0),
    }
}

fn build_tab_indexes(items: &[ProjectRequirementItem]) -> Vec<Vec<usize>> {
    ProjectRequirementsTab::ALL
        .iter()
        .map(|tab| {
            items
                .iter()
                .enumerate()
                .filter(|(_, item)| tab.matches(item.kind))
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

fn default_row(indexes: &[usize], checked: &[bool]) -> usize {
    indexes
        .iter()
        .position(|&index| checked[index])
        .unwrap_or(0)
}

fn reset_rows_to_matches(
    rows: &mut [usize],
    tab_indexes: &[Vec<usize>],
    items: &[ProjectRequirementItem],
    checked: &[bool],
    search: &str,
) {
    for (row, indexes) in rows.iter_mut().zip(tab_indexes) {
        let filtered = filtered_indexes(items, indexes, search);
        *row = if filtered.is_empty() {
            0
        } else {
            filtered
                .iter()
                .position(|&index| checked[index])
                .unwrap_or(0)
        };
    }
}

fn filtered_indexes(
    items: &[ProjectRequirementItem],
    indexes: &[usize],
    search: &str,
) -> Vec<usize> {
    if search.is_empty() {
        return indexes.to_vec();
    }

    let search_lower = search.to_lowercase();
    indexes
        .iter()
        .copied()
        .filter(|&index| {
            items[index]
                .search_corpus
                .to_lowercase()
                .contains(&search_lower)
        })
        .collect()
}

fn collect_selection(
    items: &[ProjectRequirementItem],
    checked: &[bool],
) -> ProjectRequirementsSelection {
    let mut selection = ProjectRequirementsSelection::default();

    for (item, &is_checked) in items.iter().zip(checked) {
        if !is_checked {
            continue;
        }
        match item.kind {
            RequirementKind::Skill => selection.skills.push(item.name.clone()),
            RequirementKind::Mcp => selection.mcps.push(item.name.clone()),
            RequirementKind::Subagent => selection.subagents.push(item.name.clone()),
        }
    }

    selection
}

fn render_search_line(search: &str) -> String {
    if search.is_empty() {
        format!(
            "  {} {}",
            style("/").dim(),
            style("type to filter across requirements...").dim()
        )
    } else {
        format!("  {} {}", style("/").cyan().bold(), search)
    }
}

fn render_tab_line(
    items: &[ProjectRequirementItem],
    checked: &[bool],
    active_tab: usize,
) -> String {
    let labels: Vec<String> = ProjectRequirementsTab::ALL
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let count = items
                .iter()
                .zip(checked)
                .filter(|(item, is_checked)| **is_checked && tab.matches(item.kind))
                .count();
            let label = format!("[{} {}]", tab.label(), count);
            if index == active_tab {
                format!("{}", style(label).green().bold())
            } else {
                format!("{}", style(label).dim())
            }
        })
        .collect();
    format!("  {}", labels.join("  "))
}

fn render_item_line(
    item: &ProjectRequirementItem,
    layout: Layout,
    is_checked: bool,
    is_selected: bool,
) -> String {
    let name = pad_str(&item.name, layout.name_width, Alignment::Left, None);
    let kind = pad_str(item.kind.label(), layout.kind_width, Alignment::Left, None);
    let origin = pad_str(&item.origin, layout.origin_width, Alignment::Left, None);
    let mut content = format!("{name}  [{kind}]  {origin}");
    if let Some(targets) = &item.targets {
        content.push_str("  -> ");
        content.push_str(targets);
    }
    if !item.description.is_empty() {
        content.push_str("  ");
        content.push_str(&item.description);
    }
    if is_selected {
        format!(
            "  {} {} {}",
            style(">").green(),
            if is_checked {
                style("[x]").green().to_string()
            } else {
                style("[ ]").dim().to_string()
            },
            style(content).bold()
        )
    } else {
        format!(
            "    {} {}",
            if is_checked {
                style("[x]").green().to_string()
            } else {
                style("[ ]").dim().to_string()
            },
            style(content).dim()
        )
    }
}

fn render_hint_line(
    items: &[ProjectRequirementItem],
    checked: &[bool],
    current_tab: ProjectRequirementsTab,
    total_in_tab: usize,
    filtered_count: usize,
) -> String {
    let selected_in_tab = items
        .iter()
        .zip(checked)
        .filter(|(item, is_checked)| **is_checked && current_tab.matches(item.kind))
        .count();
    let count_label = if filtered_count == total_in_tab {
        format!("{total_in_tab} entries")
    } else {
        format!("{filtered_count} of {total_in_tab} entries")
    };
    format!(
        "  {}",
        style(format!(
            "{}  ·  {} selected in {}  ·  <- -> switch tab  up/down move  space toggle  enter save  esc cancel",
            count_label,
            selected_in_tab,
            current_tab.label()
        ))
        .dim()
    )
}

fn truncation_tail(max_width: usize) -> &'static str {
    match max_width {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn clamp_line_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    truncate_str(line, max_width, truncation_tail(max_width)).into_owned()
}

fn write_clamped_line(term: &Term, line: String, max_width: usize) -> io::Result<()> {
    term.write_line(&clamp_line_width(&line, max_width))
}

fn clear_drawn_block(term: &Term, lines: usize) -> io::Result<()> {
    term.move_cursor_up(lines)?;
    for _ in 0..lines {
        term.clear_line()?;
        term.move_cursor_down(1)?;
    }
    term.move_cursor_up(lines)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use arc_core::capability::{McpDefinition, McpTransportType, SubagentDefinition};
    use arc_core::models::{SkillEntry, SkillOrigin};

    use super::{
        ProjectRequirementItem, ProjectRequirementsSelection, RequirementKind, build_items,
        collect_selection, render_tab_line,
    };
    use arc_core::mcp_registry::{McpCatalogEntry, McpEntryOrigin};
    use arc_core::subagent_registry::{SubagentCatalogEntry, SubagentEntryOrigin};

    fn sample_skill(name: &str, summary: &str) -> SkillEntry {
        SkillEntry {
            name: name.to_string(),
            origin: SkillOrigin::BuiltIn,
            summary: summary.to_string(),
            source_path: std::path::PathBuf::from(format!("/tmp/{name}/SKILL.md")),
            installed_targets: Vec::new(),
            market_repo: None,
        }
    }

    fn sample_mcp(name: &str) -> McpCatalogEntry {
        McpCatalogEntry {
            definition: McpDefinition {
                name: name.to_string(),
                targets: Some(vec!["codex".to_string()]),
                transport: McpTransportType::Stdio,
                command: None,
                args: Vec::new(),
                env: BTreeMap::new(),
                cwd: None,
                env_file: None,
                url: None,
                headers: BTreeMap::new(),
                timeout: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled: None,
                required: None,
                trust: None,
                include_tools: Vec::new(),
                exclude_tools: Vec::new(),
                oauth: None,
                description: Some("MCP description".to_string()),
            },
            origin: McpEntryOrigin::Builtin,
        }
    }

    fn sample_subagent(name: &str) -> SubagentCatalogEntry {
        SubagentCatalogEntry {
            definition: SubagentDefinition {
                name: name.to_string(),
                description: Some("Subagent description".to_string()),
                targets: Some(vec!["claude".to_string()]),
                prompt_file: format!("/tmp/{name}.md"),
            },
            origin: SubagentEntryOrigin::User,
            prompt_body: "Prompt".to_string(),
        }
    }

    #[test]
    fn build_items_keeps_missing_defaults() {
        let defaults = ProjectRequirementsSelection {
            skills: vec!["ghost-skill".to_string()],
            mcps: vec!["ghost-mcp".to_string()],
            subagents: vec!["ghost-subagent".to_string()],
        };

        let items = build_items(&[], &[], &[], &defaults);

        assert_eq!(items.len(), 3);
        assert!(items.iter().any(|item| {
            item.name == "ghost-skill"
                && item.kind == RequirementKind::Skill
                && item.origin == "missing from catalog"
                && item.default_selected
        }));
        assert!(items.iter().any(|item| {
            item.name == "ghost-mcp"
                && item.kind == RequirementKind::Mcp
                && item.origin == "missing from catalog"
                && item.default_selected
        }));
        assert!(items.iter().any(|item| {
            item.name == "ghost-subagent"
                && item.kind == RequirementKind::Subagent
                && item.origin == "missing from catalog"
                && item.default_selected
        }));
    }

    #[test]
    fn collect_selection_groups_names_by_kind() {
        let defaults = ProjectRequirementsSelection {
            skills: vec!["alpha".to_string()],
            mcps: vec!["filesystem".to_string()],
            subagents: vec!["reviewer".to_string()],
        };
        let items = build_items(
            &[
                sample_skill("alpha", "Architecture"),
                sample_skill("beta", "Beta"),
            ],
            &[sample_mcp("filesystem")],
            &[sample_subagent("reviewer")],
            &defaults,
        );
        let checked: Vec<bool> = items.iter().map(|item| item.default_selected).collect();

        let selection = collect_selection(&items, &checked);

        assert_eq!(selection.skills, vec!["alpha".to_string()]);
        assert_eq!(selection.mcps, vec!["filesystem".to_string()]);
        assert_eq!(selection.subagents, vec!["reviewer".to_string()]);
    }

    #[test]
    fn tab_line_shows_selected_counts() {
        let items = vec![
            ProjectRequirementItem {
                name: "alpha".to_string(),
                kind: RequirementKind::Skill,
                origin: "built-in".to_string(),
                targets: None,
                description: String::new(),
                search_corpus: "alpha".to_string(),
                default_selected: false,
            },
            ProjectRequirementItem {
                name: "filesystem".to_string(),
                kind: RequirementKind::Mcp,
                origin: "built-in".to_string(),
                targets: None,
                description: String::new(),
                search_corpus: "filesystem".to_string(),
                default_selected: false,
            },
        ];
        let checked = vec![true, false];

        let line = render_tab_line(&items, &checked, 0);

        assert!(line.contains("[All 1]"));
        assert!(line.contains("[Skills 1]"));
        assert!(line.contains("[MCPs 0]"));
        assert!(line.contains("[Subagents 0]"));
    }
}
