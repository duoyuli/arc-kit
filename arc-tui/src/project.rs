use std::collections::HashSet;
use std::io;

use arc_core::models::SkillEntry;
use console::{Alignment, Key, Term, measure_text_width, pad_str, style, truncate_str};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectRequirementsSelection {
    pub skills: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProjectRequirementItem {
    name: String,
    origin: String,
    description: String,
    search_corpus: String,
    default_selected: bool,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    name_width: usize,
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
) -> io::Result<Option<ProjectRequirementsSelection>> {
    run_project_requirements_editor_with_defaults(skills, &ProjectRequirementsSelection::default())
}

pub fn run_project_requirements_editor_with_defaults(
    skills: &[SkillEntry],
    defaults: &ProjectRequirementsSelection,
) -> io::Result<Option<ProjectRequirementsSelection>> {
    let items = build_items(skills, defaults);
    let layout = measure_layout(&items);
    let mut checked: Vec<bool> = items.iter().map(|item| item.default_selected).collect();
    let mut row = default_row(&checked);
    let mut scroll = 0usize;
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

        let filtered = filtered_indexes(&items, &search);
        if filtered.is_empty() {
            row = 0;
            scroll = 0;
        } else {
            row = row.min(filtered.len() - 1);
            if row < scroll {
                scroll = row;
            } else if row >= scroll + visible_rows {
                scroll = row + 1 - visible_rows;
            }
        }

        write_clamped_line(
            &term,
            format!(
                "  {}  {}",
                style("Project Skills").bold(),
                style(format!(
                    "{} selected",
                    checked.iter().filter(|&&v| v).count()
                ))
                .dim()
            ),
            max_line_width,
        )?;
        write_clamped_line(&term, render_search_line(&search), max_line_width)?;

        let shown = if filtered.is_empty() {
            write_clamped_line(
                &term,
                format!(
                    "  {}",
                    style("No matching skills. Backspace to clear filter.").dim()
                ),
                max_line_width,
            )?;
            1
        } else {
            let mut shown = 0usize;
            for (pos, &item_idx) in filtered.iter().enumerate().skip(scroll).take(visible_rows) {
                shown += 1;
                write_clamped_line(
                    &term,
                    render_item_line(&items[item_idx], layout, checked[item_idx], pos == row),
                    max_line_width,
                )?;
            }
            shown
        };

        write_clamped_line(
            &term,
            render_hint_line(&checked, items.len(), filtered.len()),
            max_line_width,
        )?;

        prev_drawn = shown + 3;
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
            Key::ArrowUp => {
                if !filtered.is_empty() {
                    row = if row == 0 {
                        filtered.len() - 1
                    } else {
                        row - 1
                    };
                }
            }
            Key::ArrowDown => {
                if !filtered.is_empty() {
                    row = (row + 1) % filtered.len();
                }
            }
            Key::Char(' ') => {
                if let Some(&item_idx) = filtered.get(row) {
                    checked[item_idx] = !checked[item_idx];
                }
            }
            Key::Backspace => {
                search.pop();
                row = default_filtered_row(&items, &checked, &search);
                scroll = 0;
            }
            Key::Char(ch) if !ch.is_ascii_control() => {
                search.push(ch);
                row = default_filtered_row(&items, &checked, &search);
                scroll = 0;
            }
            _ => {}
        }
    }
}

fn build_items(
    skills: &[SkillEntry],
    defaults: &ProjectRequirementsSelection,
) -> Vec<ProjectRequirementItem> {
    let default_skills: HashSet<&str> = defaults.skills.iter().map(String::as_str).collect();
    let known_skill_names: HashSet<&str> = skills.iter().map(|entry| entry.name.as_str()).collect();
    let mut items = Vec::new();

    for entry in skills {
        items.push(ProjectRequirementItem {
            name: entry.name.clone(),
            origin: entry.origin_display(),
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
        items.push(missing_item(name));
    }

    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.origin.cmp(&b.origin)));
    items
}

fn missing_item(name: &str) -> ProjectRequirementItem {
    let origin = "missing from catalog".to_string();
    ProjectRequirementItem {
        name: name.to_string(),
        origin: origin.clone(),
        description: String::new(),
        search_corpus: format!("{name} Skill {origin}"),
        default_selected: true,
    }
}

fn skill_search_corpus(entry: &SkillEntry) -> String {
    let mut out = format!("{} Skill {}", entry.name, entry.origin_display());
    if !entry.summary.is_empty() {
        out.push(' ');
        out.push_str(&entry.summary);
    }
    out
}

fn measure_layout(items: &[ProjectRequirementItem]) -> Layout {
    Layout {
        name_width: items
            .iter()
            .map(|item| measure_text_width(&item.name))
            .max()
            .unwrap_or(0),
        origin_width: items
            .iter()
            .map(|item| measure_text_width(&item.origin))
            .max()
            .unwrap_or(0),
    }
}

fn default_row(checked: &[bool]) -> usize {
    checked
        .iter()
        .position(|is_checked| *is_checked)
        .unwrap_or(0)
}

fn default_filtered_row(items: &[ProjectRequirementItem], checked: &[bool], search: &str) -> usize {
    let filtered = filtered_indexes(items, search);
    if filtered.is_empty() {
        0
    } else {
        filtered
            .iter()
            .position(|&index| checked[index])
            .unwrap_or(0)
    }
}

fn filtered_indexes(items: &[ProjectRequirementItem], search: &str) -> Vec<usize> {
    if search.is_empty() {
        return (0..items.len()).collect();
    }

    let search_lower = search.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.search_corpus.to_lowercase().contains(&search_lower))
        .map(|(index, _)| index)
        .collect()
}

fn collect_selection(
    items: &[ProjectRequirementItem],
    checked: &[bool],
) -> ProjectRequirementsSelection {
    let mut selection = ProjectRequirementsSelection::default();

    for (item, &is_checked) in items.iter().zip(checked) {
        if is_checked {
            selection.skills.push(item.name.clone());
        }
    }

    selection
}

fn render_search_line(search: &str) -> String {
    if search.is_empty() {
        format!(
            "  {} {}",
            style("/").dim(),
            style("type to filter skills...").dim()
        )
    } else {
        format!("  {} {}", style("/").cyan().bold(), search)
    }
}

fn render_item_line(
    item: &ProjectRequirementItem,
    layout: Layout,
    is_checked: bool,
    is_selected: bool,
) -> String {
    let name = pad_str(&item.name, layout.name_width, Alignment::Left, None);
    let origin = pad_str(&item.origin, layout.origin_width, Alignment::Left, None);
    let mut content = format!("{name}  {origin}");
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

fn render_hint_line(checked: &[bool], total: usize, filtered_count: usize) -> String {
    let selected = checked.iter().filter(|&&value| value).count();
    let count_label = if filtered_count == total {
        format!("{total} skills")
    } else {
        format!("{filtered_count} of {total} skills")
    };
    format!(
        "  {}",
        style(format!(
            "{}  ·  {} selected  ·  up/down move  space toggle  enter save  esc cancel",
            count_label, selected
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
    use arc_core::models::{SkillEntry, SkillOrigin};

    use super::{
        ProjectRequirementItem, ProjectRequirementsSelection, build_items, collect_selection,
    };

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

    #[test]
    fn build_items_keeps_missing_skill_defaults() {
        let defaults = ProjectRequirementsSelection {
            skills: vec!["ghost-skill".to_string()],
        };

        let items = build_items(&[], &defaults);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "ghost-skill");
        assert_eq!(items[0].origin, "missing from catalog");
        assert!(items[0].default_selected);
    }

    #[test]
    fn collect_selection_returns_checked_skills() {
        let defaults = ProjectRequirementsSelection {
            skills: vec!["alpha".to_string()],
        };
        let items = build_items(
            &[
                sample_skill("alpha", "Architecture"),
                sample_skill("beta", "Beta"),
            ],
            &defaults,
        );
        let checked: Vec<bool> = items.iter().map(|item| item.default_selected).collect();

        let selection = collect_selection(&items, &checked);

        assert_eq!(selection.skills, vec!["alpha".to_string()]);
    }

    #[test]
    fn unchecked_items_are_not_selected() {
        let items = vec![ProjectRequirementItem {
            name: "alpha".to_string(),
            origin: "built-in".to_string(),
            description: String::new(),
            search_corpus: "alpha".to_string(),
            default_selected: false,
        }];

        let selection = collect_selection(&items, &[false]);

        assert!(selection.skills.is_empty());
    }
}
