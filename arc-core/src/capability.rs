use std::collections::{BTreeMap, BTreeSet, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::agent::{
    AppliedResourceScope, McpConfigFormat, McpScopeSupport, McpTransportSupport, SubagentFormat,
    SubagentSupport, agent_mcp_path, agent_spec, agent_subagent_dir,
    ordered_agent_ids_for_resource_kind,
};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::models::ResourceKind;
use crate::paths::{ArcPaths, expand_user_path};

static RESOURCE_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9-_]{0,63}$").expect("valid resource regex"));
static ENV_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\$\{[A-Z0-9_]+\}$").expect("valid env regex"));
static AUTH_PLACEHOLDER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(Bearer|Basic)\s+\$\{[A-Z0-9_]+\}$").expect("valid auth env regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransportType {
    Stdio,
    Sse,
    StreamableHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeFallback {
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceScope {
    Global,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceResolution {
    Active,
    Shadowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTargetState {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredScope {
    Project,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedScope {
    Project,
    Global,
    None,
}

impl AppliedScope {
    pub fn from_tracking(scope: AppliedResourceScope) -> Self {
        match scope {
            AppliedResourceScope::Project => Self::Project,
            AppliedResourceScope::Global | AppliedResourceScope::GlobalFallback => Self::Global,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    pub transport: McpTransportType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_fallback: Option<ScopeFallback>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    pub prompt_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedCapabilityInstall {
    pub kind: ResourceKind,
    pub name: String,
    pub agent: String,
    pub source_scope: SourceScope,
    pub applied_scope: AppliedResourceScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityTargetStatus {
    pub agent: String,
    pub status: CapabilityTargetState,
    pub desired_scope: DesiredScope,
    pub applied_scope: AppliedScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityStatusEntry {
    pub name: String,
    pub kind: ResourceKind,
    pub source_scope: SourceScope,
    pub managed_by_arc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_targets: Option<Vec<String>>,
    pub resolution: ResourceResolution,
    pub targets: Vec<CapabilityTargetStatus>,
}

#[derive(Debug, Clone)]
pub struct McpApplyPlan {
    pub definition: McpDefinition,
    pub source_scope: SourceScope,
}

#[derive(Debug, Clone)]
pub struct SubagentApplyPlan {
    pub definition: SubagentDefinition,
    pub prompt_path: PathBuf,
    pub source_scope: SourceScope,
}

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
        AppliedScope::Global => {
            if source_scope == SourceScope::Project {
                AppliedResourceScope::GlobalFallback
            } else {
                AppliedResourceScope::Global
            }
        }
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
        ResourceKind::Mcp => mcp_install_present(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            record.project_root.as_deref(),
        ),
        ResourceKind::SubAgent => subagent_install_present(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            record.project_root.as_deref(),
        ),
        _ => Ok(false),
    }
}

pub fn validate_mcp_definition(
    definition: &mut McpDefinition,
    source_scope: SourceScope,
) -> Result<()> {
    normalize_targets(&mut definition.targets);
    validate_resource_name(&definition.name, "mcp")?;
    validate_declared_targets(definition.targets.as_ref(), &ResourceKind::Mcp)?;
    if source_scope == SourceScope::Global && definition.scope_fallback.is_some() {
        return Err(ArcError::new(
            "scope_fallback is only allowed on project mcps",
        ));
    }
    match definition.transport {
        McpTransportType::Stdio => {
            if definition.command.as_deref().unwrap_or("").is_empty() {
                return Err(ArcError::new(format!(
                    "mcp '{}' requires command for stdio transport",
                    definition.name
                )));
            }
            if definition.url.is_some() {
                return Err(ArcError::new(format!(
                    "mcp '{}' cannot set url for stdio transport",
                    definition.name
                )));
            }
        }
        McpTransportType::Sse | McpTransportType::StreamableHttp => {
            if definition.url.as_deref().unwrap_or("").is_empty() {
                return Err(ArcError::new(format!(
                    "mcp '{}' requires url for remote transport",
                    definition.name
                )));
            }
            if definition.command.is_some() {
                return Err(ArcError::new(format!(
                    "mcp '{}' cannot set command for remote transport",
                    definition.name
                )));
            }
        }
    }
    validate_secret_map(&definition.env)?;
    validate_secret_map(&definition.headers)?;
    Ok(())
}

pub fn validate_subagent_definition(
    definition: &mut SubagentDefinition,
    source_scope: SourceScope,
    base_dir: &Path,
) -> Result<PathBuf> {
    normalize_targets(&mut definition.targets);
    validate_resource_name(&definition.name, "subagent")?;
    validate_declared_targets(definition.targets.as_ref(), &ResourceKind::SubAgent)?;
    let prompt_path = if source_scope == SourceScope::Global {
        expand_user_path(&definition.prompt_file)
    } else {
        base_dir.join(&definition.prompt_file)
    };
    if !prompt_path.is_file() {
        return Err(ArcError::new(format!(
            "subagent '{}' prompt_file not found: {}",
            definition.name,
            prompt_path.display()
        )));
    }
    Ok(prompt_path)
}

pub fn load_global_mcps(paths: &ArcPaths) -> Result<Vec<McpDefinition>> {
    let mut entries = Vec::new();
    let dir = paths.mcps_dir();
    let Ok(items) = fs::read_dir(&dir) else {
        return Ok(entries);
    };
    for item in items.flatten() {
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let body = fs::read_to_string(&path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut definition: McpDefinition = toml::from_str(&body)
            .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))?;
        validate_mcp_definition(&mut definition, SourceScope::Global)?;
        entries.push(definition);
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn save_global_mcp(paths: &ArcPaths, definition: &McpDefinition) -> Result<()> {
    let mut normalized = definition.clone();
    validate_mcp_definition(&mut normalized, SourceScope::Global)?;
    let path = paths.mcps_dir().join(format!("{}.toml", normalized.name));
    let body = toml::to_string_pretty(&normalized).map_err(|e| {
        ArcError::new(format!(
            "failed to serialize mcp '{}': {e}",
            normalized.name
        ))
    })?;
    fs::write(&path, body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

pub fn remove_global_mcp(paths: &ArcPaths, name: &str) -> Result<()> {
    let path = paths.mcps_dir().join(format!("{name}.toml"));
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|e| ArcError::new(format!("failed to remove {}: {e}", path.display())))?;
    Ok(())
}

pub fn load_global_subagents(paths: &ArcPaths) -> Result<Vec<SubagentDefinition>> {
    let mut entries = Vec::new();
    let dir = paths.subagents_dir();
    let Ok(items) = fs::read_dir(&dir) else {
        return Ok(entries);
    };
    for item in items.flatten() {
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let body = fs::read_to_string(&path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut definition: SubagentDefinition = toml::from_str(&body)
            .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))?;
        let prompt_path = paths
            .subagents_dir()
            .join(format!("{}.md", definition.name));
        definition.prompt_file = prompt_path.display().to_string();
        validate_subagent_definition(&mut definition, SourceScope::Global, paths.home())?;
        entries.push(definition);
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn load_global_subagent_prompt(paths: &ArcPaths, name: &str) -> Result<String> {
    let prompt_path = paths.subagents_dir().join(format!("{name}.md"));
    fs::read_to_string(&prompt_path)
        .map_err(|e| ArcError::new(format!("failed to read {}: {e}", prompt_path.display())))
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
    normalize_targets(&mut normalized.targets);
    validate_resource_name(&normalized.name, "subagent")?;
    validate_declared_targets(normalized.targets.as_ref(), &ResourceKind::SubAgent)?;
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
    fs::write(&meta_path, metadata)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", meta_path.display())))?;
    fs::write(&prompt_path, prompt_body)
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
    fs::write(&path, body)
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

pub fn apply_mcp_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
    allow_global_fallback: bool,
) -> Result<Vec<CapabilityTargetStatus>> {
    let tracked = list_tracked_capability_installs(paths);
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let mut statuses = Vec::new();

    for agent in targets {
        let status = evaluate_mcp_target(
            paths,
            cache,
            &tracked,
            &agent,
            plan,
            project_root,
            allow_global_fallback,
            true,
        )?;
        if let Some(tracking) = tracking_record_for_target(
            ResourceKind::Mcp,
            &plan.definition.name,
            plan.source_scope,
            &status,
            project_root,
        ) {
            track_capability_install(paths, &tracking)?;
        }
        statuses.push(status);
    }

    Ok(statuses)
}

pub fn apply_subagent_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let prompt_body = fs::read_to_string(&plan.prompt_path).map_err(|e| {
        ArcError::new(format!(
            "failed to read subagent prompt {}: {e}",
            plan.prompt_path.display()
        ))
    })?;
    let mut statuses = Vec::new();

    for agent in targets {
        let status =
            evaluate_subagent_target(paths, cache, &agent, plan, project_root, &prompt_body, true)?;
        if let Some(tracking) = tracking_record_for_target(
            ResourceKind::SubAgent,
            &plan.definition.name,
            plan.source_scope,
            &status,
            project_root,
        ) {
            track_capability_install(paths, &tracking)?;
        }
        statuses.push(status);
    }

    Ok(statuses)
}

pub fn preview_mcp_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    tracked: &[TrackedCapabilityInstall],
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
    allow_global_fallback: bool,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    targets
        .into_iter()
        .map(|agent| {
            evaluate_mcp_target(
                paths,
                cache,
                tracked,
                &agent,
                plan,
                project_root,
                allow_global_fallback,
                false,
            )
        })
        .collect()
}

pub fn preview_subagent_plan(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
) -> Result<Vec<CapabilityTargetStatus>> {
    let targets = resolve_declared_targets(cache, plan.definition.targets.as_ref());
    let prompt_body = fs::read_to_string(&plan.prompt_path).map_err(|e| {
        ArcError::new(format!(
            "failed to read subagent prompt {}: {e}",
            plan.prompt_path.display()
        ))
    })?;
    targets
        .into_iter()
        .map(|agent| {
            evaluate_subagent_target(
                paths,
                cache,
                &agent,
                plan,
                project_root,
                &prompt_body,
                false,
            )
        })
        .collect()
}

pub fn remove_tracked_capability(
    paths: &ArcPaths,
    record: &TrackedCapabilityInstall,
    project_root: Option<&Path>,
) -> Result<()> {
    match record.kind {
        ResourceKind::Mcp => remove_agent_mcp(
            paths,
            &record.agent,
            &record.name,
            record.applied_scope,
            project_root,
        )?,
        ResourceKind::SubAgent => remove_agent_subagent(
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

pub fn resolve_declared_targets(
    cache: &DetectCache,
    declared_targets: Option<&Vec<String>>,
) -> Vec<String> {
    if let Some(targets) = declared_targets {
        return dedupe_targets(targets.clone());
    }
    cache.detected_agents().keys().cloned().collect()
}

pub fn is_shadowed(name: &str, project_names: &BTreeSet<String>) -> bool {
    project_names.contains(name)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_mcp_target(
    paths: &ArcPaths,
    cache: &DetectCache,
    tracked: &[TrackedCapabilityInstall],
    agent: &str,
    plan: &McpApplyPlan,
    project_root: Option<&Path>,
    allow_global_fallback: bool,
    perform_write: bool,
) -> Result<CapabilityTargetStatus> {
    let Some(agent_info) = cache.get_agent(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    };
    if agent_info.root.is_none() {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    }

    let Some(spec) = agent_spec(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_mcp".to_string()),
        });
    };

    if !supports_mcp_transport(spec.mcp_transport_support, plan.definition.transport) {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_transport".to_string()),
        });
    }

    let (desired_scope, applied_scope) = match plan.source_scope {
        SourceScope::Global => (DesiredScope::Global, AppliedResourceScope::Global),
        SourceScope::Project => match spec.mcp_scope_support {
            McpScopeSupport::ProjectNative => {
                (DesiredScope::Project, AppliedResourceScope::Project)
            }
            McpScopeSupport::GlobalOnly => {
                if allow_global_fallback
                    || plan.definition.scope_fallback == Some(ScopeFallback::Global)
                {
                    if tracked.iter().any(|record| {
                        record.kind == ResourceKind::Mcp
                            && record.name == plan.definition.name
                            && record.agent == agent
                            && record.source_scope == SourceScope::Global
                    }) {
                        return Ok(CapabilityTargetStatus {
                            agent: agent.to_string(),
                            status: CapabilityTargetState::Failed,
                            desired_scope: DesiredScope::Global,
                            applied_scope: AppliedScope::None,
                            reason: Some("name_conflict_with_global".to_string()),
                        });
                    }
                    (DesiredScope::Global, AppliedResourceScope::GlobalFallback)
                } else {
                    return Ok(CapabilityTargetStatus {
                        agent: agent.to_string(),
                        status: CapabilityTargetState::Skipped,
                        desired_scope: DesiredScope::Project,
                        applied_scope: AppliedScope::None,
                        reason: Some("requires_global_fallback".to_string()),
                    });
                }
            }
            McpScopeSupport::Unsupported => {
                return Ok(CapabilityTargetStatus {
                    agent: agent.to_string(),
                    status: CapabilityTargetState::Skipped,
                    desired_scope: DesiredScope::Project,
                    applied_scope: AppliedScope::None,
                    reason: Some("unsupported_mcp".to_string()),
                });
            }
        },
    };

    if perform_write {
        write_agent_mcp(paths, agent, &plan.definition, applied_scope, project_root)?;
    }
    Ok(CapabilityTargetStatus {
        agent: agent.to_string(),
        status: CapabilityTargetState::Applied,
        desired_scope,
        applied_scope: AppliedScope::from_tracking(applied_scope),
        reason: None,
    })
}

fn evaluate_subagent_target(
    paths: &ArcPaths,
    cache: &DetectCache,
    agent: &str,
    plan: &SubagentApplyPlan,
    project_root: Option<&Path>,
    prompt_body: &str,
    perform_write: bool,
) -> Result<CapabilityTargetStatus> {
    let Some(agent_info) = cache.get_agent(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    };
    if agent_info.root.is_none() {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("agent_not_detected".to_string()),
        });
    }
    let Some(spec) = agent_spec(agent) else {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_subagent".to_string()),
        });
    };
    if spec.subagent_support != SubagentSupport::Native {
        return Ok(CapabilityTargetStatus {
            agent: agent.to_string(),
            status: CapabilityTargetState::Skipped,
            desired_scope: if plan.source_scope == SourceScope::Project {
                DesiredScope::Project
            } else {
                DesiredScope::Global
            },
            applied_scope: AppliedScope::None,
            reason: Some("unsupported_subagent".to_string()),
        });
    }
    let applied_scope = if plan.source_scope == SourceScope::Project {
        AppliedResourceScope::Project
    } else {
        AppliedResourceScope::Global
    };
    if perform_write {
        write_agent_subagent(
            paths,
            agent,
            &plan.definition,
            prompt_body,
            applied_scope,
            project_root,
        )?;
    }
    Ok(CapabilityTargetStatus {
        agent: agent.to_string(),
        status: CapabilityTargetState::Applied,
        desired_scope: if plan.source_scope == SourceScope::Project {
            DesiredScope::Project
        } else {
            DesiredScope::Global
        },
        applied_scope: AppliedScope::from_tracking(applied_scope),
        reason: None,
    })
}

fn write_agent_mcp(
    paths: &ArcPaths,
    agent: &str,
    definition: &McpDefinition,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(spec) = agent_spec(agent) else {
        return Err(ArcError::new(format!("unknown agent '{agent}'")));
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Err(ArcError::new(format!(
            "mcp path unavailable for agent '{agent}'"
        )));
    };
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapMcpServers) => {
            upsert_json_map_mcp(&path, "mcpServers", definition)
        }
        Some(McpConfigFormat::JsonOpenCode) => upsert_opencode_mcp(&path, definition),
        Some(McpConfigFormat::TomlMcpServers) => upsert_toml_mcp(&path, definition),
        None => Err(ArcError::new(format!(
            "agent '{agent}' does not support mcp"
        ))),
    }
}

fn remove_agent_mcp(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(());
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Ok(());
    };
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapMcpServers) => remove_json_map_mcp(&path, "mcpServers", name),
        Some(McpConfigFormat::JsonOpenCode) => remove_opencode_mcp(&path, name),
        Some(McpConfigFormat::TomlMcpServers) => remove_toml_mcp(&path, name),
        None => Ok(()),
    }
}

fn write_agent_subagent(
    paths: &ArcPaths,
    agent: &str,
    definition: &SubagentDefinition,
    prompt_body: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Err(ArcError::new(format!(
            "subagent directory unavailable for agent '{agent}'"
        )));
    };
    fs::create_dir_all(&dir)
        .map_err(|e| ArcError::new(format!("failed to create {}: {e}", dir.display())))?;
    let Some(spec) = agent_spec(agent) else {
        return Err(ArcError::new(format!("unknown agent '{agent}'")));
    };
    let Some(format) = spec.subagent_format else {
        return Err(ArcError::new(format!(
            "subagent writer not implemented for agent '{agent}'"
        )));
    };
    match format {
        SubagentFormat::TomlDeveloperInstructions => {
            let file = dir.join(format!("{}.toml", sanitize_filename(&definition.name)));
            let mut table = toml::map::Map::new();
            table.insert(
                "name".to_string(),
                toml::Value::String(definition.name.clone()),
            );
            if let Some(description) = &definition.description {
                table.insert(
                    "description".to_string(),
                    toml::Value::String(description.clone()),
                );
            }
            table.insert(
                "developer_instructions".to_string(),
                toml::Value::String(prompt_body.to_string()),
            );
            let body = toml::to_string_pretty(&toml::Value::Table(table))
                .map_err(|e| ArcError::new(format!("failed to serialize codex subagent: {e}")))?;
            fs::write(&file, body)
                .map_err(|e| ArcError::new(format!("failed to write {}: {e}", file.display())))?;
        }
        SubagentFormat::MarkdownFrontmatter => {
            let file = dir.join(format!("{}.md", sanitize_filename(&definition.name)));
            let body = render_markdown_subagent(definition, prompt_body)?;
            fs::write(&file, body)
                .map_err(|e| ArcError::new(format!("failed to write {}: {e}", file.display())))?;
        }
    }
    Ok(())
}

fn remove_agent_subagent(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<()> {
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Ok(());
    };
    let Some(spec) = agent_spec(agent) else {
        return Ok(());
    };
    let file = match spec.subagent_format {
        Some(SubagentFormat::TomlDeveloperInstructions) => {
            dir.join(format!("{}.toml", sanitize_filename(name)))
        }
        Some(SubagentFormat::MarkdownFrontmatter) => {
            dir.join(format!("{}.md", sanitize_filename(name)))
        }
        _ => return Ok(()),
    };
    if file.exists() {
        fs::remove_file(&file)
            .map_err(|e| ArcError::new(format!("failed to remove {}: {e}", file.display())))?;
    }
    Ok(())
}

fn render_markdown_subagent(definition: &SubagentDefinition, prompt_body: &str) -> Result<String> {
    #[derive(Serialize)]
    struct Frontmatter<'a> {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<&'a str>,
    }

    let frontmatter = serde_yaml::to_string(&Frontmatter {
        name: &definition.name,
        description: definition.description.as_deref(),
    })
    .map_err(|e| ArcError::new(format!("failed to serialize subagent frontmatter: {e}")))?;
    Ok(format!("---\n{}---\n{}", frontmatter, prompt_body))
}

fn upsert_json_map_mcp(path: &Path, key: &str, definition: &McpDefinition) -> Result<()> {
    let mut root = load_json_root(path)?;
    set_nested_json_object(
        &mut root,
        key,
        &definition.name,
        serde_json::to_value(json_map_mcp_value(definition))
            .map_err(|e| ArcError::new(format!("failed to serialize mcp json: {e}")))?,
    )?;
    write_json_root(path, &root)
}

fn remove_json_map_mcp(path: &Path, key: &str, name: &str) -> Result<()> {
    let mut root = load_json_root(path)?;
    remove_nested_json_key(&mut root, key, name)?;
    write_json_root(path, &root)
}

fn upsert_opencode_mcp(path: &Path, definition: &McpDefinition) -> Result<()> {
    let mut root = load_json_root(path)?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| ArcError::new(format!("expected JSON object in {}", path.display())))?;
    let mcp = obj
        .entry("mcp")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let mcp_obj = mcp
        .as_object_mut()
        .ok_or_else(|| ArcError::new(format!("expected object at mcp in {}", path.display())))?;
    mcp_obj.insert(definition.name.clone(), opencode_mcp_value(definition));
    write_json_root(path, &root)
}

fn remove_opencode_mcp(path: &Path, name: &str) -> Result<()> {
    let mut root = load_json_root(path)?;
    if let Some(obj) = root.as_object_mut()
        && let Some(mcp) = obj.get_mut("mcp")
        && let Some(mcp_obj) = mcp.as_object_mut()
    {
        mcp_obj.remove(name);
    }
    write_json_root(path, &root)
}

fn upsert_toml_mcp(path: &Path, definition: &McpDefinition) -> Result<()> {
    if definition.transport != McpTransportType::Stdio {
        return Err(ArcError::new(format!(
            "agent config at {} only supports stdio mcp entries",
            path.display()
        )));
    }
    let mut root = load_toml_root(path)?;
    let table = root
        .as_table_mut()
        .ok_or_else(|| ArcError::new(format!("expected TOML table in {}", path.display())))?;
    let mcp_servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let server_table = mcp_servers.as_table_mut().ok_or_else(|| {
        ArcError::new(format!(
            "expected TOML table at mcp_servers in {}",
            path.display()
        ))
    })?;
    server_table.insert(definition.name.clone(), toml_stdio_mcp_value(definition)?);
    write_toml_root(path, &root)
}

fn remove_toml_mcp(path: &Path, name: &str) -> Result<()> {
    let mut root = load_toml_root(path)?;
    if let Some(table) = root.as_table_mut()
        && let Some(mcp_servers) = table.get_mut("mcp_servers")
        && let Some(servers) = mcp_servers.as_table_mut()
    {
        servers.remove(name);
    }
    write_toml_root(path, &root)
}

fn json_map_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => serde_json::json!({
            "type": "stdio",
            "command": definition.command.clone().unwrap_or_default(),
            "args": definition.args,
            "env": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
        }),
        McpTransportType::Sse => serde_json::json!({
            "type": "sse",
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
        }),
        McpTransportType::StreamableHttp => serde_json::json!({
            "type": "http",
            "url": definition.url.clone().unwrap_or_default(),
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
        }),
    }
}

fn opencode_mcp_value(definition: &McpDefinition) -> serde_json::Value {
    match definition.transport {
        McpTransportType::Stdio => {
            let mut command = vec![definition.command.clone().unwrap_or_default()];
            command.extend(definition.args.clone());
            serde_json::json!({
                "type": "local",
                "command": command,
                "enabled": true,
                "environment": if definition.env.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.env).unwrap_or(serde_json::Value::Null) },
            })
        }
        McpTransportType::Sse | McpTransportType::StreamableHttp => serde_json::json!({
            "type": "remote",
            "url": definition.url.clone().unwrap_or_default(),
            "enabled": true,
            "headers": if definition.headers.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&definition.headers).unwrap_or(serde_json::Value::Null) },
        }),
    }
}

fn toml_stdio_mcp_value(definition: &McpDefinition) -> Result<toml::Value> {
    let command = definition.command.clone().ok_or_else(|| {
        ArcError::new(format!(
            "mcp '{}' requires command for stdio transport",
            definition.name
        ))
    })?;
    let mut table = toml::map::Map::new();
    table.insert("command".to_string(), toml::Value::String(command));
    if !definition.args.is_empty() {
        table.insert(
            "args".to_string(),
            toml::Value::Array(
                definition
                    .args
                    .iter()
                    .map(|arg| toml::Value::String(arg.clone()))
                    .collect(),
            ),
        );
    }
    if !definition.env.is_empty() {
        let env_table = definition
            .env
            .iter()
            .map(|(key, value)| (key.clone(), toml::Value::String(value.clone())))
            .collect();
        table.insert("env".to_string(), toml::Value::Table(env_table));
    }
    Ok(toml::Value::Table(table))
}

fn load_json_root(path: &Path) -> Result<serde_json::Value> {
    match fs::read_to_string(path) {
        Ok(body) => {
            if body.trim().is_empty() {
                Ok(serde_json::Value::Object(serde_json::Map::new()))
            } else {
                serde_json::from_str(&body)
                    .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(serde_json::Value::Object(serde_json::Map::new()))
        }
        Err(err) => Err(ArcError::new(format!(
            "failed to read {}: {err}",
            path.display()
        ))),
    }
}

fn write_json_root(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ArcError::new(format!("failed to create {}: {e}", parent.display())))?;
    }
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| ArcError::new(format!("failed to serialize {}: {e}", path.display())))?;
    fs::write(path, body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

fn load_toml_root(path: &Path) -> Result<toml::Value> {
    match fs::read_to_string(path) {
        Ok(body) => {
            if body.trim().is_empty() {
                Ok(toml::Value::Table(toml::map::Map::new()))
            } else {
                toml::from_str(&body)
                    .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(toml::Value::Table(toml::map::Map::new()))
        }
        Err(err) => Err(ArcError::new(format!(
            "failed to read {}: {err}",
            path.display()
        ))),
    }
}

fn write_toml_root(path: &Path, value: &toml::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ArcError::new(format!("failed to create {}: {e}", parent.display())))?;
    }
    let body = toml::to_string_pretty(value)
        .map_err(|e| ArcError::new(format!("failed to serialize {}: {e}", path.display())))?;
    fs::write(path, body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

fn set_nested_json_object(
    root: &mut serde_json::Value,
    path: &str,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    let keys: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for segment in keys {
        let object = current
            .as_object_mut()
            .ok_or_else(|| ArcError::new("expected JSON object while writing nested mcp config"))?;
        current = object
            .entry(segment)
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    }
    let object = current
        .as_object_mut()
        .ok_or_else(|| ArcError::new("expected JSON object while writing mcp entry"))?;
    object.insert(key.to_string(), value);
    Ok(())
}

fn remove_nested_json_key(root: &mut serde_json::Value, path: &str, key: &str) -> Result<()> {
    let keys: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for segment in keys {
        let Some(next) = current.get_mut(segment) else {
            return Ok(());
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.remove(key);
    }
    Ok(())
}

fn mcp_install_present(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<bool> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(false);
    };
    let Some(path) = agent_mcp_path(paths, agent, scope, project_root) else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }
    match spec.mcp_config_format {
        Some(McpConfigFormat::JsonMapMcpServers) => {
            let root = load_json_root(&path)?;
            Ok(json_nested_contains_key(&root, "mcpServers", name))
        }
        Some(McpConfigFormat::JsonOpenCode) => {
            let root = load_json_root(&path)?;
            Ok(root
                .get("mcp")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|mcp| mcp.contains_key(name)))
        }
        Some(McpConfigFormat::TomlMcpServers) => {
            let root = load_toml_root(&path)?;
            Ok(root
                .as_table()
                .and_then(|table| table.get("mcp_servers"))
                .and_then(toml::Value::as_table)
                .is_some_and(|servers| servers.contains_key(name)))
        }
        None => Ok(false),
    }
}

fn subagent_install_present(
    paths: &ArcPaths,
    agent: &str,
    name: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Result<bool> {
    let Some(spec) = agent_spec(agent) else {
        return Ok(false);
    };
    let Some(dir) = agent_subagent_dir(paths, agent, scope, project_root) else {
        return Ok(false);
    };
    let file = match spec.subagent_format {
        Some(SubagentFormat::TomlDeveloperInstructions) => {
            dir.join(format!("{}.toml", sanitize_filename(name)))
        }
        Some(SubagentFormat::MarkdownFrontmatter) => {
            dir.join(format!("{}.md", sanitize_filename(name)))
        }
        None => return Ok(false),
    };
    Ok(file.exists())
}

fn json_nested_contains_key(root: &serde_json::Value, path: &str, key: &str) -> bool {
    let mut current = root;
    for segment in path.split('.') {
        let Some(next) = current.get(segment) else {
            return false;
        };
        current = next;
    }
    current
        .as_object()
        .is_some_and(|object| object.contains_key(key))
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

fn validate_resource_name(name: &str, kind: &str) -> Result<()> {
    if RESOURCE_NAME_RE.is_match(name) {
        return Ok(());
    }
    Err(ArcError::new(format!(
        "{kind} name '{name}' must match ^[a-z0-9][a-z0-9-_]{{0,63}}$"
    )))
}

fn validate_secret_map(map: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in map {
        if is_secret_key(key) && !is_secret_placeholder_value(value) {
            return Err(ArcError::new(format!(
                "secret field '{key}' must use an environment placeholder"
            )));
        }
    }
    Ok(())
}

fn validate_declared_targets(targets: Option<&Vec<String>>, kind: &ResourceKind) -> Result<()> {
    let Some(targets) = targets else {
        return Ok(());
    };
    let supported = ordered_agent_ids_for_resource_kind(kind);
    for target in targets {
        if supported.iter().any(|item| item == target) {
            continue;
        }
        return Err(ArcError::with_hint(
            format!("unsupported target agent '{target}' for {}", kind.as_str()),
            format!("Available: {}", supported.join(", ")),
        ));
    }
    Ok(())
}

fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered == "authorization"
        || lowered.contains("token")
        || lowered.contains("secret")
        || lowered.contains("key")
        || lowered.contains("cookie")
}

fn is_secret_placeholder_value(value: &str) -> bool {
    ENV_PLACEHOLDER_RE.is_match(value) || AUTH_PLACEHOLDER_RE.is_match(value)
}

fn normalize_targets(targets: &mut Option<Vec<String>>) {
    let Some(items) = targets.as_mut() else {
        return;
    };
    let mut seen = BTreeSet::new();
    items.retain(|item| !item.is_empty() && seen.insert(item.clone()));
    if items.is_empty() {
        *targets = None;
    }
}

fn dedupe_targets(targets: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .filter(|item| !item.is_empty() && seen.insert(item.clone()))
        .collect()
}

fn supports_mcp_transport(support: McpTransportSupport, transport: McpTransportType) -> bool {
    match transport {
        McpTransportType::Stdio => support.supports_stdio,
        McpTransportType::Sse => support.supports_sse,
        McpTransportType::StreamableHttp => support.supports_streamable_http,
    }
}

fn sanitize_filename(name: &str) -> String {
    let collapsed = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_ascii_lowercase();
    collapsed
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_validation_accepts_placeholders() {
        let mut definition = McpDefinition {
            name: "github".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer ${GITHUB_TOKEN}".to_string(),
            )]),
            description: None,
            scope_fallback: None,
        };

        assert!(validate_mcp_definition(&mut definition, SourceScope::Global).is_ok());
    }

    #[test]
    fn secret_validation_rejects_plaintext() {
        let mut definition = McpDefinition {
            name: "github".to_string(),
            targets: None,
            transport: McpTransportType::StreamableHttp,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: Some("https://example.com/mcp".to_string()),
            headers: BTreeMap::from([(
                "Authorization".to_string(),
                "Bearer ghp_secret".to_string(),
            )]),
            description: None,
            scope_fallback: None,
        };

        assert!(validate_mcp_definition(&mut definition, SourceScope::Global).is_err());
    }
}
