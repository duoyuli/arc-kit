use crate::interact_required_multi_select;
use arc_core::models::CatalogResource;

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
    let prompt = format!("Select {}s", kind.replace('_', " "));
    let selected_resource_indexes =
        interact_required_multi_select(&prompt, &resource_labels, None)?;
    let selected_resources = selected_resource_indexes
        .into_iter()
        .filter_map(|index| resources.get(index).map(|resource| resource.name.clone()))
        .collect();
    let selected_agent_indexes = interact_required_multi_select("Select agents", agents, None)?;
    let selected_agents = selected_agent_indexes
        .into_iter()
        .filter_map(|index| agents.get(index).cloned())
        .collect();
    Ok((selected_resources, selected_agents))
}
