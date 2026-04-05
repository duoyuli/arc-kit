use arc_core::detect::coding_agent_spec;
use dialoguer::{MultiSelect, Select};

use crate::theme::theme;

pub(crate) fn agent_display_name(id: &str) -> String {
    coding_agent_spec(id)
        .map(|spec| spec.display_name.to_string())
        .unwrap_or_else(|| id.to_string())
}

pub fn select_agent(agents: &[String], installed: &[&String]) -> dialoguer::Result<Option<String>> {
    match agents.len() {
        0 => Ok(None),
        1 => Ok(Some(agents[0].clone())),
        _ => {
            let labels: Vec<String> = agents
                .iter()
                .map(|id| {
                    let name = agent_display_name(id);
                    if installed.contains(&id) {
                        format!("{name}  ✓")
                    } else {
                        name
                    }
                })
                .collect();
            let idx = Select::with_theme(&theme())
                .with_prompt("Agent")
                .items(&labels)
                .default(0)
                .interact_opt()?;
            Ok(idx.map(|i| agents[i].clone()))
        }
    }
}

pub fn select_agents(agents: &[String], installed: &[&String]) -> dialoguer::Result<Vec<String>> {
    if agents.is_empty() {
        return Ok(Vec::new());
    }
    if agents.len() == 1 {
        return Ok(vec![agents[0].clone()]);
    }
    let labels: Vec<String> = agents.iter().map(|id| agent_display_name(id)).collect();
    let defaults: Vec<bool> = agents.iter().map(|id| installed.contains(&id)).collect();
    let selected_indexes = MultiSelect::with_theme(&theme())
        .with_prompt("Agent")
        .items(&labels)
        .defaults(&defaults)
        .interact()?;
    Ok(selected_indexes
        .into_iter()
        .filter_map(|i| agents.get(i).cloned())
        .collect())
}
