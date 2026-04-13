//! Text layout shared by `arc provider list` and tests.

use std::collections::HashMap;

use arc_core::agent::agent_spec;
use arc_core::provider::{ProviderInfo, supported_provider_agents};
use console::measure_text_width;
#[cfg(test)]
use console::{Alignment, pad_str};

#[derive(Debug, Clone)]
pub enum ProviderListLine {
    AgentHeader {
        agent_display: String,
    },
    ProviderRow {
        is_active: bool,
        display_name: String,
        description: String,
        name_width: usize,
    },
}

pub fn build_provider_list_lines(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> Vec<ProviderListLine> {
    let mut lines = Vec::new();
    for agent in supported_provider_agents() {
        let group: Vec<&ProviderInfo> = providers.iter().filter(|p| p.agent == *agent).collect();
        if group.is_empty() {
            continue;
        }
        let agent_display = agent_spec(agent)
            .map(|s| s.display_name.to_string())
            .unwrap_or_else(|| agent.to_string());
        lines.push(ProviderListLine::AgentHeader { agent_display });

        let name_width = group
            .iter()
            .map(|p| measure_text_width(&p.display_name))
            .max()
            .unwrap_or(0);

        for provider in group {
            let is_active = active_providers
                .get(&provider.agent)
                .is_some_and(|name| name == &provider.name);
            lines.push(ProviderListLine::ProviderRow {
                is_active,
                display_name: provider.display_name.clone(),
                description: provider.description.clone(),
                name_width,
            });
        }
    }
    lines
}

#[cfg(test)]
pub fn format_provider_list_line_plain(line: &ProviderListLine) -> String {
    match line {
        ProviderListLine::AgentHeader { agent_display } => format!("  {agent_display}"),
        ProviderListLine::ProviderRow {
            is_active,
            display_name,
            description,
            name_width,
        } => {
            let marker = if *is_active { "✓" } else { " " };
            if description.is_empty() {
                format!("    {marker} {display_name}")
            } else {
                let padded = pad_str(display_name, *name_width, Alignment::Left, None);
                format!("    {marker} {padded}  {description}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_core::provider::{CodexProviderConfig, ProviderSettings};

    #[test]
    fn list_lines_group_and_plain_match_list_layout() {
        let providers = vec![
            ProviderInfo {
                name: "p1".to_string(),
                display_name: "Short".to_string(),
                description: String::new(),
                agent: "codex".to_string(),
                settings: ProviderSettings::Codex(CodexProviderConfig::default()),
            },
            ProviderInfo {
                name: "p2".to_string(),
                display_name: "Longer Name".to_string(),
                description: "desc".to_string(),
                agent: "codex".to_string(),
                settings: ProviderSettings::Codex(CodexProviderConfig::default()),
            },
        ];
        let mut active = HashMap::new();
        active.insert("codex".to_string(), "p1".to_string());
        let lines = build_provider_list_lines(&providers, &active);
        let plain: Vec<_> = lines.iter().map(format_provider_list_line_plain).collect();
        assert_eq!(plain[0], "  Codex");
        assert_eq!(plain[1], "    ✓ Short");
        assert!(plain[2].contains("Longer Name"));
        assert!(plain[2].contains("desc"));
        assert!(plain[2].starts_with("    "));
    }
}
