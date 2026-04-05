//! Text layout shared by `arc provider` list and the interactive provider picker.

use std::collections::HashMap;

use console::{Alignment, measure_text_width, pad_str};

use crate::detect::coding_agent_spec;
use crate::provider::{ProviderInfo, supported_provider_agents};

#[derive(Debug, Clone)]
pub enum ProviderListLine {
    AgentHeader {
        agent_display: String,
    },
    ProviderRow {
        provider_index: usize,
        is_active: bool,
        display_name: String,
        description: String,
        name_width: usize,
    },
}

/// Builds the same line sequence as the text UI of `arc provider list` (grouped by agent).
pub fn build_provider_list_lines(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> Vec<ProviderListLine> {
    let mut lines = Vec::new();
    for agent in supported_provider_agents() {
        let group: Vec<(usize, &ProviderInfo)> = providers
            .iter()
            .enumerate()
            .filter(|(_, p)| p.agent == *agent)
            .collect();
        if group.is_empty() {
            continue;
        }
        let agent_display = coding_agent_spec(agent)
            .map(|s| s.display_name.to_string())
            .unwrap_or_else(|| agent.to_string());
        lines.push(ProviderListLine::AgentHeader { agent_display });

        let name_width = group
            .iter()
            .map(|(_, p)| measure_text_width(&p.display_name))
            .max()
            .unwrap_or(0);

        for (idx, p) in group {
            let is_active = active_providers
                .get(&p.agent)
                .is_some_and(|name| name == &p.name);
            lines.push(ProviderListLine::ProviderRow {
                provider_index: idx,
                is_active,
                display_name: p.display_name.clone(),
                description: p.description.clone(),
                name_width,
            });
        }
    }
    lines
}

pub fn format_provider_list_line_plain(line: &ProviderListLine) -> String {
    match line {
        ProviderListLine::AgentHeader { agent_display } => format!("  {agent_display}"),
        ProviderListLine::ProviderRow {
            is_active,
            display_name,
            description,
            name_width,
            ..
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

/// Same text layout as `arc provider list` (bold agent titles, dim descriptions).
pub fn print_provider_list_lines_stdout(lines: &[ProviderListLine]) {
    use console::style;

    for line in lines {
        match line {
            ProviderListLine::AgentHeader { agent_display } => {
                println!();
                println!("  {}", style(agent_display).bold());
            }
            ProviderListLine::ProviderRow {
                is_active,
                display_name,
                description,
                name_width,
                ..
            } => {
                let marker = if *is_active { "✓" } else { " " };
                if description.is_empty() {
                    println!("    {marker} {}", display_name);
                } else {
                    let padded = pad_str(display_name, *name_width, Alignment::Left, None);
                    println!("    {marker} {padded}  {}", style(description).dim(),);
                }
            }
        }
    }
    println!();
}

pub fn provider_list_line_mapping(lines: &[ProviderListLine]) -> Vec<Option<usize>> {
    lines
        .iter()
        .map(|l| match l {
            ProviderListLine::AgentHeader { .. } => None,
            ProviderListLine::ProviderRow { provider_index, .. } => Some(*provider_index),
        })
        .collect()
}

/// Default row index for `dialoguer::Select` when items include agent headers + provider rows.
pub fn default_provider_list_row(
    mapping: &[Option<usize>],
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> usize {
    let preferred = providers.iter().position(|p| {
        active_providers
            .get(&p.agent)
            .is_some_and(|name| name == &p.name)
    });
    if let Some(pi) = preferred {
        for (row, m) in mapping.iter().enumerate() {
            if *m == Some(pi) {
                return row;
            }
        }
    }
    mapping.iter().position(|m| m.is_some()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::provider::{
        CodexProviderConfig, ProviderInfo, ProviderSettings, build_provider_list_lines,
        default_provider_list_row, format_provider_list_line_plain, provider_list_line_mapping,
    };

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

    #[test]
    fn default_row_targets_active_provider() {
        let providers = vec![
            ProviderInfo {
                name: "a".to_string(),
                display_name: "A".to_string(),
                description: String::new(),
                agent: "codex".to_string(),
                settings: ProviderSettings::Codex(CodexProviderConfig::default()),
            },
            ProviderInfo {
                name: "b".to_string(),
                display_name: "B".to_string(),
                description: String::new(),
                agent: "codex".to_string(),
                settings: ProviderSettings::Codex(CodexProviderConfig::default()),
            },
        ];
        let mut active = HashMap::new();
        active.insert("codex".to_string(), "b".to_string());
        let lines = build_provider_list_lines(&providers, &active);
        let mapping = provider_list_line_mapping(&lines);
        let def = default_provider_list_row(&mapping, &providers, &active);
        assert_eq!(mapping[def], Some(1));
    }
}
