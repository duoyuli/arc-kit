use std::io;

use arc_core::subagent_registry::{SubagentCatalogEntry, SubagentEntryOrigin};
use dialoguer::Input;

use crate::agent::agent_display_name;
use crate::fuzzy::{browse_list, fuzzy_select_opt};
use crate::select_agents;
use crate::theme::theme;

pub fn run_subagent_install_wizard(
    available_agents: &[String],
    default_name: Option<&str>,
    default_description: Option<&str>,
    default_prompt_file: Option<&str>,
    default_agents: &[String],
) -> dialoguer::Result<(String, Option<String>, String, Vec<String>)> {
    let name = Input::<String>::with_theme(&theme())
        .with_prompt("Subagent name")
        .with_initial_text(default_name.unwrap_or_default())
        .validate_with(|input: &String| -> Result<(), &'static str> {
            if input.trim().is_empty() {
                Err("Subagent name is required.")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let description = Input::<String>::with_theme(&theme())
        .with_prompt("Description (optional)")
        .with_initial_text(default_description.unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;

    let prompt_file = Input::<String>::with_theme(&theme())
        .with_prompt("Prompt file")
        .with_initial_text(default_prompt_file.unwrap_or_default())
        .validate_with(|input: &String| -> Result<(), &'static str> {
            if input.trim().is_empty() {
                Err("Prompt file is required.")
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let installed: Vec<&String> = default_agents.iter().collect();
    let agents = select_agents(available_agents, &installed)?;

    Ok((
        name.trim().to_string(),
        if description.trim().is_empty() {
            None
        } else {
            Some(description.trim().to_string())
        },
        prompt_file.trim().to_string(),
        agents,
    ))
}

pub fn pick_subagent(entries: &[SubagentCatalogEntry]) -> io::Result<Option<String>> {
    let (display_labels, search_corpus) = browser_data(entries);
    let selected = fuzzy_select_opt(&display_labels, &search_corpus)?;

    Ok(selected.and_then(|index| {
        entries
            .get(index)
            .map(|entry| entry.definition.name.clone())
    }))
}

pub fn run_subagent_browser<F>(entries: &[SubagentCatalogEntry], render_detail: F) -> io::Result<()>
where
    F: Fn(&SubagentCatalogEntry),
{
    let (display_labels, search_corpus) = browser_data(entries);
    browse_list(entries, &display_labels, &search_corpus, render_detail)
}

fn browser_data(entries: &[SubagentCatalogEntry]) -> (Vec<String>, Vec<String>) {
    let name_width = entries
        .iter()
        .map(|entry| entry.definition.name.len())
        .max()
        .unwrap_or(0);
    let display_labels = entries
        .iter()
        .map(|entry| subagent_label(entry, name_width))
        .collect();
    let search_corpus = entries.iter().map(subagent_search_corpus).collect();

    (display_labels, search_corpus)
}

fn subagent_label(entry: &SubagentCatalogEntry, name_width: usize) -> String {
    let base = format!(
        "{:<width$}  [{}]",
        entry.definition.name,
        origin_label(&entry.origin),
        width = name_width
    );
    match targets_label(entry.definition.targets.as_ref()) {
        Some(targets) => format!("{base}  → {targets}"),
        None => base,
    }
}

fn subagent_search_corpus(entry: &SubagentCatalogEntry) -> String {
    let mut out = format!(
        "{} {} {}",
        entry.definition.name,
        origin_label(&entry.origin),
        entry.definition.prompt_file
    );
    if let Some(targets) = targets_label(entry.definition.targets.as_ref()) {
        out.push(' ');
        out.push_str(&targets);
    }
    if let Some(description) = &entry.definition.description {
        out.push(' ');
        out.push_str(description);
    }
    out
}

fn origin_label(origin: &SubagentEntryOrigin) -> &'static str {
    match origin {
        SubagentEntryOrigin::Builtin => "built-in",
        SubagentEntryOrigin::User => "user",
    }
}

fn targets_label(targets: Option<&Vec<String>>) -> Option<String> {
    targets.map(|targets| {
        targets
            .iter()
            .map(|id| agent_display_name(id))
            .collect::<Vec<_>>()
            .join(", ")
    })
}
