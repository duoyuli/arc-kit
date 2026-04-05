use arc_core::detect::coding_agent_spec;

pub fn agent_display_name(agent_id: &str) -> &str {
    coding_agent_spec(agent_id)
        .map(|spec| spec.display_name)
        .unwrap_or(agent_id)
}

pub fn agent_display_names(agent_ids: &[String]) -> String {
    agent_ids
        .iter()
        .map(|id| agent_display_name(id).to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
