use std::collections::HashMap;

use arc_core::provider::{
    ProviderInfo, ProviderListLine, build_provider_list_lines, default_provider_list_row,
    format_provider_list_line_plain, provider_list_line_mapping,
};
use console::style;
use dialoguer::Select;

use crate::theme::theme;

/// Same interaction model as other arc-tui prompts (`dialoguer` on stderr, no alternate screen).
pub fn select_provider(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> dialoguer::Result<Option<ProviderInfo>> {
    if providers.is_empty() {
        return Ok(None);
    }
    let lines = build_provider_list_lines(providers, active_providers);
    if lines.is_empty() {
        return Ok(None);
    }

    let labels: Vec<String> = lines.iter().map(list_line_label_for_select).collect();
    let mapping = provider_list_line_mapping(&lines);
    let default_idx = default_provider_list_row(&mapping, providers, active_providers);

    loop {
        let index = Select::with_theme(&theme())
            .with_prompt("Provider")
            .items(&labels)
            .default(default_idx)
            .interact_opt()?;
        let Some(i) = index else {
            return Ok(None);
        };
        match mapping[i] {
            Some(pi) => return Ok(Some(providers[pi].clone())),
            None => continue,
        }
    }
}

fn list_line_label_for_select(line: &ProviderListLine) -> String {
    match line {
        ProviderListLine::AgentHeader { agent_display } => {
            format!("{}", style(format!("  {agent_display}")).bold())
        }
        _ => format_provider_list_line_plain(line),
    }
}
