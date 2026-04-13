use super::*;

pub(super) fn collect_agents(
    paths: &ArcPaths,
    cache: &DetectCache,
    skill_counts: &BTreeMap<String, usize>,
) -> Vec<AgentRuntimeStatus> {
    let providers_dir = paths.providers_dir();

    cache
        .detected_agents()
        .iter()
        .map(|(agent_id, info)| {
            let spec = agent_spec(agent_id);
            let provider = read_active_provider(&providers_dir, agent_id).map(|active_name| {
                let display_name = load_providers_for_agent(&providers_dir, agent_id)
                    .into_iter()
                    .find(|p| p.name == active_name)
                    .map(|p| p.display_name)
                    .unwrap_or_else(|| active_name.clone());
                AgentProviderStatus {
                    name: active_name,
                    display_name,
                }
            });

            AgentRuntimeStatus {
                id: agent_id.clone(),
                name: spec
                    .map(|item| item.display_name.to_string())
                    .unwrap_or_else(|| agent_id.clone()),
                version: info.version.clone(),
                provider,
                global_skill_count: skill_counts.get(agent_id).copied().unwrap_or(0),
                supports_project_skills: spec.is_some_and(|item| item.supports_project_skills),
                supports_provider: supports_provider_agent(agent_id),
                mcp_scope_supported: spec
                    .map(|item| match item.mcp_scope_support {
                        McpScopeSupport::ProjectNative => "project_native",
                        McpScopeSupport::GlobalOnly => "global_only",
                        McpScopeSupport::Unsupported => "unsupported",
                    })
                    .unwrap_or("unsupported")
                    .to_string(),
                mcp_transports_supported: spec
                    .map(|item| {
                        let mut transports = Vec::new();
                        if item.mcp_transport_support.supports_stdio {
                            transports.push("stdio".to_string());
                        }
                        if item.mcp_transport_support.supports_sse {
                            transports.push("sse".to_string());
                        }
                        if item.mcp_transport_support.supports_streamable_http {
                            transports.push("streamable_http".to_string());
                        }
                        transports
                    })
                    .unwrap_or_default(),
                subagent_supported: spec
                    .map(|item| match item.subagent_support {
                        SubagentSupport::Native => "native",
                        SubagentSupport::Unsupported => "unsupported",
                    })
                    .unwrap_or("unsupported")
                    .to_string(),
            }
        })
        .collect()
}

pub(super) fn count_skills_by_agent(items: &[InstalledResource]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for item in items {
        if item.kind.as_str() != "skill" {
            continue;
        }
        for target in &item.targets {
            *counts.entry(target.clone()).or_insert(0) += 1;
        }
    }
    counts
}
