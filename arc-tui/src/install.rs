use arc_core::models::CatalogResource;
use dialoguer::MultiSelect;

use crate::theme::theme;

pub fn run_install_wizard(
    kind: &str,
    resources: &[CatalogResource],
    agents: &[String],
) -> dialoguer::Result<(Vec<String>, Vec<String>)> {
    let resource_labels: Vec<String> = resources
        .iter()
        .map(|resource| {
            if resource.installed {
                format!("{} ({kind}, installed)", resource.name)
            } else {
                format!("{} ({kind})", resource.name)
            }
        })
        .collect();
    let selected_resource_indexes = MultiSelect::with_theme(&theme())
        .with_prompt(format!("Select {}s", kind.replace('_', " ")))
        .items(&resource_labels)
        .interact()?;
    if selected_resource_indexes.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let selected_resources = selected_resource_indexes
        .into_iter()
        .filter_map(|index| resources.get(index).map(|resource| resource.name.clone()))
        .collect();
    let selected_agent_indexes = MultiSelect::with_theme(&theme())
        .with_prompt("Select agents")
        .items(agents)
        .interact()?;
    let selected_agents = selected_agent_indexes
        .into_iter()
        .filter_map(|index| agents.get(index).cloned())
        .collect();
    Ok((selected_resources, selected_agents))
}
