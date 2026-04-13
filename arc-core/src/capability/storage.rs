use super::*;
use std::hash::{DefaultHasher, Hash, Hasher};

pub fn tracking_record_for_target(
    kind: ResourceKind,
    name: &str,
    source_scope: SourceScope,
    target: &CapabilityTargetStatus,
    project_root: Option<&Path>,
) -> Option<TrackedCapabilityInstall> {
    if target.status != CapabilityTargetState::Applied {
        return None;
    }
    let applied_scope = match target.applied_scope {
        AppliedScope::Project => AppliedResourceScope::Project,
        AppliedScope::Global => AppliedResourceScope::Global,
        AppliedScope::None => return None,
    };
    Some(TrackedCapabilityInstall {
        kind,
        name: name.to_string(),
        agent: target.agent.clone(),
        source_scope,
        applied_scope,
        project_root: project_root.map(Path::to_path_buf),
    })
}

pub fn capability_install_present(
    paths: &ArcPaths,
    record: &TrackedCapabilityInstall,
) -> Result<bool> {
    match record.kind {
        ResourceKind::Mcp => agent_config::mcp_install_present(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            record.project_root.as_deref(),
        ),
        ResourceKind::SubAgent => agent_config::subagent_install_present(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            record.project_root.as_deref(),
        ),
        _ => Ok(false),
    }
}

pub fn load_global_mcps(paths: &ArcPaths) -> Result<Vec<McpDefinition>> {
    let catalog = mcp_registry::load_merged_mcp_catalog(paths)?;
    let mut entries: Vec<McpDefinition> = catalog.into_iter().map(|e| e.definition).collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn save_global_mcp(paths: &ArcPaths, definition: &McpDefinition) -> Result<()> {
    let mut normalized = definition.clone();
    validate::validate_mcp_definition(&mut normalized)?;
    mcp_registry::upsert_user_registry_mcp(paths, &normalized)
}

pub fn remove_global_mcp(paths: &ArcPaths, name: &str) -> Result<()> {
    let _ = mcp_registry::remove_user_registry_mcp(paths, name)?;
    Ok(())
}

pub fn load_global_subagents(paths: &ArcPaths) -> Result<Vec<SubagentDefinition>> {
    let mut entries: Vec<SubagentDefinition> =
        subagent_registry::load_merged_subagent_catalog(paths)?
            .into_iter()
            .map(|entry| entry.definition)
            .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn load_global_subagent_prompt(paths: &ArcPaths, name: &str) -> Result<String> {
    let Some(entry) = subagent_registry::find_global_subagent(paths, name)? else {
        return Err(ArcError::new(format!("subagent '{name}' not found")));
    };
    Ok(entry.prompt_body)
}

pub fn save_global_subagent(
    paths: &ArcPaths,
    definition: &SubagentDefinition,
    prompt_body: &str,
) -> Result<()> {
    let mut normalized = definition.clone();
    normalized.prompt_file = paths
        .subagents_dir()
        .join(format!("{}.md", normalized.name))
        .display()
        .to_string();
    validate::normalize_targets(&mut normalized.targets);
    validate::validate_resource_name(&normalized.name, "subagent")?;
    validate::validate_declared_targets(normalized.targets.as_ref(), &ResourceKind::SubAgent)?;
    let meta_path = paths
        .subagents_dir()
        .join(format!("{}.toml", normalized.name));
    let prompt_path = paths
        .subagents_dir()
        .join(format!("{}.md", normalized.name));
    let metadata = toml::to_string_pretty(&normalized).map_err(|e| {
        ArcError::new(format!(
            "failed to serialize subagent '{}': {e}",
            normalized.name
        ))
    })?;
    atomic_write_string(&meta_path, &metadata)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", meta_path.display())))?;
    atomic_write_string(&prompt_path, prompt_body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", prompt_path.display())))?;
    Ok(())
}

pub fn remove_global_subagent(paths: &ArcPaths, name: &str) -> Result<()> {
    for path in [
        paths.subagents_dir().join(format!("{name}.toml")),
        paths.subagents_dir().join(format!("{name}.md")),
    ] {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| ArcError::new(format!("failed to remove {}: {e}", path.display())))?;
        }
    }
    Ok(())
}

pub fn list_tracked_capability_installs(paths: &ArcPaths) -> Vec<TrackedCapabilityInstall> {
    let Ok(items) = fs::read_dir(paths.tracking_dir()) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for item in items.flatten() {
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(body) = fs::read(&path) else {
            continue;
        };
        if let Ok(record) = serde_json::from_slice::<TrackedCapabilityInstall>(&body) {
            records.push(record);
        }
    }
    records
}

pub fn track_capability_install(paths: &ArcPaths, record: &TrackedCapabilityInstall) -> Result<()> {
    let path = tracked_record_path(paths, record);
    let body = serde_json::to_vec_pretty(record)
        .map_err(|e| ArcError::new(format!("failed to serialize tracking record: {e}")))?;
    atomic_write_bytes(&path, &body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

pub fn untrack_capability_install(
    paths: &ArcPaths,
    record: &TrackedCapabilityInstall,
) -> Result<()> {
    let path = tracked_record_path(paths, record);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| ArcError::new(format!("failed to remove {}: {e}", path.display())))?;
    }
    Ok(())
}

pub fn remove_tracked_capability(
    paths: &ArcPaths,
    record: &TrackedCapabilityInstall,
    project_root: Option<&Path>,
) -> Result<()> {
    match record.kind {
        ResourceKind::Mcp => agent_config::remove_agent_mcp(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            project_root,
        )?,
        ResourceKind::SubAgent => agent_config::remove_agent_subagent(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            project_root,
        )?,
        _ => {}
    }
    untrack_capability_install(paths, record)
}

pub(crate) fn load_subagent_prompt_body(plan: &SubagentApplyPlan) -> Result<String> {
    if let Some(prompt_body) = &plan.prompt_body {
        return Ok(prompt_body.clone());
    }

    let prompt_path = plan
        .prompt_path
        .as_ref()
        .ok_or_else(|| ArcError::new("subagent prompt source missing"))?;
    fs::read_to_string(prompt_path).map_err(|e| {
        ArcError::new(format!(
            "failed to read subagent prompt {}: {e}",
            prompt_path.display()
        ))
    })
}

fn tracked_record_path(paths: &ArcPaths, record: &TrackedCapabilityInstall) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    record.kind.hash(&mut hasher);
    record.name.hash(&mut hasher);
    record.agent.hash(&mut hasher);
    record.source_scope.hash(&mut hasher);
    record.applied_scope.hash(&mut hasher);
    record.project_root.hash(&mut hasher);
    let digest = hasher.finish();
    paths
        .tracking_dir()
        .join(format!("capability-{digest:016x}.json"))
}
