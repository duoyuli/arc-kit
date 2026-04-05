use std::collections::HashSet;
use std::io;

use arc_core::models::SkillEntry;
use dialoguer::console;

use crate::agent::agent_display_name;
use crate::fuzzy::{browse_list, fuzzy_multi_select, fuzzy_select_opt};
use crate::interact_required_multi_select;

/// Pick skills only (for `arc project apply` / `require` list). No agent selection.
pub fn run_skill_require_pick_wizard(skills: &[SkillEntry]) -> dialoguer::Result<Vec<String>> {
    run_skill_require_pick_wizard_with_defaults(skills, &[])
}

/// Same as [`run_skill_require_pick_wizard`], but marks `preselected` names as initially selected.
pub fn run_skill_require_pick_wizard_with_defaults(
    skills: &[SkillEntry],
    preselected: &[String],
) -> dialoguer::Result<Vec<String>> {
    let pre: HashSet<&str> = preselected.iter().map(|s| s.as_str()).collect();
    let name_width = skills.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let display_labels: Vec<String> = skills.iter().map(|s| skill_label(s, name_width)).collect();
    let search_corpus: Vec<String> = skills.iter().map(skill_search_corpus).collect();

    let defaults: Vec<bool> = skills
        .iter()
        .map(|s| pre.contains(s.name.as_str()))
        .collect();
    let selected_indices = fuzzy_multi_select(&display_labels, &search_corpus, &defaults)?;
    let Some(indices) = selected_indices else {
        return Ok(Vec::new());
    };
    if indices.is_empty() {
        return Ok(Vec::new());
    }
    let selected_skills: Vec<&SkillEntry> = indices.iter().filter_map(|&i| skills.get(i)).collect();
    Ok(selected_skills.iter().map(|s| s.name.clone()).collect())
}

pub fn run_skill_install_wizard(
    skills: &[SkillEntry],
    agents: &[String],
) -> dialoguer::Result<(Vec<String>, Vec<String>)> {
    let selected_names = run_skill_require_pick_wizard(skills)?;
    if selected_names.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let selected_skills: Vec<&SkillEntry> = selected_names
        .iter()
        .filter_map(|n| skills.iter().find(|s| &s.name == n))
        .collect();

    let mut installable: Vec<&String> = Vec::new();
    let mut installed_labels: Vec<String> = Vec::new();
    for id in agents {
        if selected_skills
            .iter()
            .all(|s| s.installed_targets.contains(id))
        {
            installed_labels.push(format!(
                "{} (installed)",
                console::style(agent_display_name(id)).dim()
            ));
        } else {
            installable.push(id);
        }
    }

    if installable.is_empty() {
        for label in &installed_labels {
            println!("  {label}");
        }
        return Ok((Vec::new(), Vec::new()));
    }

    let agent_labels: Vec<String> = installable
        .iter()
        .map(|id| agent_display_name(id))
        .collect();
    for label in &installed_labels {
        println!("  {label}");
    }
    let selected_agent_indexes = interact_required_multi_select("Agent", &agent_labels, None)?;
    let selected_agents = selected_agent_indexes
        .into_iter()
        .filter_map(|index| installable.get(index).map(|id| (*id).clone()))
        .collect();
    Ok((selected_names, selected_agents))
}

pub fn run_skill_uninstall_wizard(
    installed: &[SkillEntry],
) -> dialoguer::Result<Option<(String, Vec<String>)>> {
    let name_width = installed.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let display_labels: Vec<String> = installed
        .iter()
        .map(|s| {
            let agent_names: Vec<String> = s
                .installed_targets
                .iter()
                .map(|id| agent_display_name(id))
                .collect();
            format!(
                "{:<width$}  → {}",
                s.name,
                agent_names.join(", "),
                width = name_width
            )
        })
        .collect();
    let search_corpus: Vec<String> = installed.iter().map(|s| s.name.clone()).collect();

    let selected = fuzzy_select_opt(&display_labels, &search_corpus)?;
    let Some(index) = selected else {
        return Ok(None);
    };
    let skill = &installed[index];

    let targets = if skill.installed_targets.len() == 1 {
        skill.installed_targets.clone()
    } else {
        let agent_labels: Vec<String> = skill
            .installed_targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect();
        let defaults = vec![true; skill.installed_targets.len()];
        let selected_indexes =
            interact_required_multi_select("Agent", &agent_labels, Some(&defaults))?;
        selected_indexes
            .into_iter()
            .filter_map(|i| skill.installed_targets.get(i).cloned())
            .collect()
    };

    Ok(Some((skill.name.clone(), targets)))
}

pub fn run_skill_browser<F>(skills: &[SkillEntry], render_detail: F) -> io::Result<()>
where
    F: Fn(&SkillEntry),
{
    let name_width = skills.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let display_labels: Vec<String> = skills.iter().map(|s| skill_label(s, name_width)).collect();
    let search_corpus: Vec<String> = skills.iter().map(skill_search_corpus).collect();
    browse_list(skills, &display_labels, &search_corpus, render_detail)
}

/// Name, summary, and origin (including market owner/repo) for fuzzy matching.
fn skill_search_corpus(entry: &SkillEntry) -> String {
    let mut out = if entry.summary.is_empty() {
        entry.name.clone()
    } else {
        format!("{} {}", entry.name, entry.summary)
    };
    out.push(' ');
    out.push_str(&entry.origin_display());
    out
}

fn skill_label(entry: &SkillEntry, name_width: usize) -> String {
    let origin = entry.origin_display();
    if entry.installed_targets.is_empty() {
        format!("{:<width$}  {origin}", entry.name, width = name_width)
    } else {
        let names = entry
            .installed_targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{:<width$}  {origin}  → {names}",
            entry.name,
            width = name_width
        )
    }
}
