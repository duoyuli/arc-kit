use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::agent::{McpScopeSupport, SubagentSupport, agent_spec, project_skill_path};
use crate::capability::{
    AppliedScope, CapabilityStatusEntry, CapabilityTargetState, CapabilityTargetStatus,
    DesiredScope, McpApplyPlan, ResourceResolution, SourceScope, SubagentApplyPlan,
    TrackedCapabilityInstall, capability_install_present, list_tracked_capability_installs,
    load_global_mcps, load_global_subagents, preview_mcp_plan, preview_subagent_plan,
    resolve_declared_targets, tracking_record_for_target, validate_mcp_definition,
    validate_subagent_definition,
};
use crate::detect::DetectCache;
use crate::engine::{InstallEngine, InstalledResource};
use crate::market::sources::MarketSourceRegistry;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::project::{find_project_config, load_project_config};
use crate::provider::{load_providers_for_agent, read_active_provider, supports_provider_agent};
use crate::skill::SkillRegistry;

#[derive(Debug, Clone, Serialize)]
pub struct StatusSnapshot {
    pub project: ProjectStatusSection,
    pub agents: Vec<AgentRuntimeStatus>,
    pub catalog: CatalogStatus,
    pub mcps: Vec<CapabilityStatusEntry>,
    pub subagents: Vec<CapabilityStatusEntry>,
    pub actions: Vec<RecommendedAction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    None,
    Invalid,
    Active,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusSection {
    pub state: ProjectState,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ProjectSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<ProjectSkillRollout>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<ProjectTargetStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProjectProviderStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub required_skills: usize,
    pub ready_skills: usize,
    pub partial_skills: usize,
    pub missing_skills: usize,
    pub unavailable_skills: usize,
    pub target_agents: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectSkillState {
    Ready,
    Partial,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSkillRollout {
    pub name: String,
    pub state: ProjectSkillState,
    pub ready_on_agents: Vec<String>,
    pub missing_on_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectTargetStatus {
    pub id: String,
    pub name: String,
    pub ready_skill_count: usize,
    pub total_available_skill_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_status: Option<ProjectProviderAgentStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectProviderStatus {
    pub name: String,
    pub matched_agents: usize,
    pub mismatched_agents: usize,
    pub missing_profiles: usize,
    pub agents: Vec<ProjectProviderAgentStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMatchState {
    Matched,
    Mismatch,
    MissingProfile,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectProviderAgentStatus {
    pub id: String,
    pub name: String,
    pub state: ProviderMatchState,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentRuntimeStatus {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProviderStatus>,
    pub global_skill_count: usize,
    pub supports_project_skills: bool,
    pub supports_provider: bool,
    pub mcp_scope_supported: String,
    pub mcp_transports_supported: Vec<String>,
    pub subagent_supported: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentProviderStatus {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogStatus {
    pub market_count: usize,
    pub resource_count: usize,
    pub global_skill_count: usize,
    pub unhealthy_market_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSeverity {
    Info,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendedAction {
    pub severity: ActionSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

pub fn collect_status(paths: &ArcPaths, cwd: &Path, cache: &DetectCache) -> StatusSnapshot {
    let engine = InstallEngine::new(cache.clone());
    let installed = engine.list_installed(Some(&ResourceKind::Skill));
    let skill_counts = count_skills_by_agent(&installed);
    let agents = collect_agents(paths, cache, &skill_counts);
    let catalog = collect_catalog(paths, installed.len());
    let project = collect_project(paths, cwd, cache, &agents);
    let tracked_capabilities = list_tracked_capability_installs(paths);
    let (mcps, subagents) = collect_capabilities(paths, cwd, cache, &tracked_capabilities);
    let actions = collect_actions(&project, &agents, &mcps, &subagents);

    StatusSnapshot {
        project,
        agents,
        catalog,
        mcps,
        subagents,
        actions,
    }
}

fn collect_agents(
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

fn collect_catalog(paths: &ArcPaths, global_skill_count: usize) -> CatalogStatus {
    let sources = MarketSourceRegistry::new(paths.clone()).list_all();
    let unhealthy_market_count = sources
        .iter()
        .filter(|source| source.status != "ok" && source.status != "indexed")
        .count();
    let resource_count = sources.iter().map(|source| source.resource_count).sum();

    CatalogStatus {
        market_count: sources.len(),
        resource_count,
        global_skill_count,
        unhealthy_market_count,
    }
}

fn collect_project(
    paths: &ArcPaths,
    cwd: &Path,
    cache: &DetectCache,
    agents: &[AgentRuntimeStatus],
) -> ProjectStatusSection {
    let config_path = find_project_config(cwd);
    let name = infer_project_name(cwd, config_path.as_deref());

    let Some(config_path) = config_path else {
        return ProjectStatusSection {
            state: ProjectState::None,
            name,
            root: None,
            config_path: None,
            error: None,
            summary: None,
            skills: Vec::new(),
            agents: Vec::new(),
            provider: None,
        };
    };

    let root = config_path.parent().map(Path::to_path_buf);
    let config = match load_project_config(&config_path) {
        Ok(config) => config,
        Err(err) => {
            return ProjectStatusSection {
                state: ProjectState::Invalid,
                name,
                root,
                config_path: Some(config_path),
                error: Some(err.to_string()),
                summary: None,
                skills: Vec::new(),
                agents: Vec::new(),
                provider: None,
            };
        }
    };

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let target_agents: Vec<&AgentRuntimeStatus> = agents
        .iter()
        .filter(|agent| agent.supports_project_skills)
        .collect();

    let skills: Vec<ProjectSkillRollout> = config
        .skills
        .require
        .iter()
        .map(|skill_name| {
            if registry.find(skill_name).is_none() {
                return ProjectSkillRollout {
                    name: skill_name.clone(),
                    state: ProjectSkillState::Unavailable,
                    ready_on_agents: Vec::new(),
                    missing_on_agents: target_agents.iter().map(|agent| agent.id.clone()).collect(),
                };
            }

            let mut ready_on_agents = Vec::new();
            let mut missing_on_agents = Vec::new();
            if let Some(project_root) = root.as_deref() {
                for agent in &target_agents {
                    let present = project_skill_path(project_root, &agent.id, skill_name)
                        .map(|path| path.exists())
                        .unwrap_or(false);
                    if present {
                        ready_on_agents.push(agent.id.clone());
                    } else {
                        missing_on_agents.push(agent.id.clone());
                    }
                }
            }

            let state = if ready_on_agents.is_empty() {
                ProjectSkillState::Missing
            } else if missing_on_agents.is_empty() {
                ProjectSkillState::Ready
            } else {
                ProjectSkillState::Partial
            };

            ProjectSkillRollout {
                name: skill_name.clone(),
                state,
                ready_on_agents,
                missing_on_agents,
            }
        })
        .collect();

    let total_available_skills = skills
        .iter()
        .filter(|skill| !matches!(skill.state, ProjectSkillState::Unavailable))
        .count();
    let project_agents: Vec<ProjectTargetStatus> = target_agents
        .iter()
        .map(|agent| {
            let ready_skill_count = skills
                .iter()
                .filter(|skill| {
                    !matches!(skill.state, ProjectSkillState::Unavailable)
                        && skill.ready_on_agents.iter().any(|id| id == &agent.id)
                })
                .count();
            ProjectTargetStatus {
                id: agent.id.clone(),
                name: agent.name.clone(),
                ready_skill_count,
                total_available_skill_count: total_available_skills,
                provider_status: None,
            }
        })
        .collect();

    let provider = config
        .provider
        .name
        .as_deref()
        .map(|provider_name| collect_project_provider(paths, agents, provider_name));
    let project_agents = attach_provider_status(project_agents, provider.as_ref());

    let summary = ProjectSummary {
        required_skills: skills.len(),
        ready_skills: skills
            .iter()
            .filter(|skill| matches!(skill.state, ProjectSkillState::Ready))
            .count(),
        partial_skills: skills
            .iter()
            .filter(|skill| matches!(skill.state, ProjectSkillState::Partial))
            .count(),
        missing_skills: skills
            .iter()
            .filter(|skill| matches!(skill.state, ProjectSkillState::Missing))
            .count(),
        unavailable_skills: skills
            .iter()
            .filter(|skill| matches!(skill.state, ProjectSkillState::Unavailable))
            .count(),
        target_agents: project_agents.len(),
    };

    ProjectStatusSection {
        state: ProjectState::Active,
        name,
        root,
        config_path: Some(config_path),
        error: None,
        summary: Some(summary),
        skills,
        agents: project_agents,
        provider,
    }
}

fn collect_project_provider(
    paths: &ArcPaths,
    agents: &[AgentRuntimeStatus],
    provider_name: &str,
) -> ProjectProviderStatus {
    let providers_dir = paths.providers_dir();
    let agent_statuses: Vec<ProjectProviderAgentStatus> = agents
        .iter()
        .filter(|agent| agent.supports_provider)
        .map(|agent| {
            let has_profile = load_providers_for_agent(&providers_dir, &agent.id)
                .iter()
                .any(|provider| provider.name == provider_name);
            let state = if !has_profile {
                ProviderMatchState::MissingProfile
            } else if read_active_provider(&providers_dir, &agent.id).as_deref()
                == Some(provider_name)
            {
                ProviderMatchState::Matched
            } else {
                ProviderMatchState::Mismatch
            };
            ProjectProviderAgentStatus {
                id: agent.id.clone(),
                name: agent.name.clone(),
                state,
            }
        })
        .collect();

    ProjectProviderStatus {
        name: provider_name.to_string(),
        matched_agents: agent_statuses
            .iter()
            .filter(|agent| matches!(agent.state, ProviderMatchState::Matched))
            .count(),
        mismatched_agents: agent_statuses
            .iter()
            .filter(|agent| matches!(agent.state, ProviderMatchState::Mismatch))
            .count(),
        missing_profiles: agent_statuses
            .iter()
            .filter(|agent| matches!(agent.state, ProviderMatchState::MissingProfile))
            .count(),
        agents: agent_statuses,
    }
}

fn attach_provider_status(
    agents: Vec<ProjectTargetStatus>,
    provider: Option<&ProjectProviderStatus>,
) -> Vec<ProjectTargetStatus> {
    let Some(provider) = provider else {
        return agents;
    };

    agents
        .into_iter()
        .map(|mut agent| {
            agent.provider_status = provider
                .agents
                .iter()
                .find(|item| item.id == agent.id)
                .cloned();
            agent
        })
        .collect()
}

fn collect_capabilities(
    paths: &ArcPaths,
    cwd: &Path,
    cache: &DetectCache,
    tracked: &[TrackedCapabilityInstall],
) -> (Vec<CapabilityStatusEntry>, Vec<CapabilityStatusEntry>) {
    let project_config =
        find_project_config(cwd).and_then(|path| match load_project_config(&path) {
            Ok(config) => path_with_root(cwd, config),
            Err(_) => None,
        });

    let project_mcp_entries = project_config
        .as_ref()
        .map(|(project_root, cfg)| {
            collect_project_mcp_entries(paths, cache, tracked, project_root, cfg)
        })
        .unwrap_or_default();
    let project_subagent_entries = project_config
        .as_ref()
        .map(|(project_root, cfg)| {
            collect_project_subagent_entries(paths, cache, project_root, cfg)
        })
        .unwrap_or_default();

    let mut mcps = Vec::new();
    match load_global_mcps(paths) {
        Ok(global_mcps) => {
            for definition in global_mcps {
                let plan = McpApplyPlan {
                    definition: definition.clone(),
                    source_scope: SourceScope::Global,
                };
                let targets = observe_capability_targets(
                    paths,
                    ResourceKind::Mcp,
                    &definition.name,
                    SourceScope::Global,
                    None,
                    preview_mcp_plan(paths, cache, tracked, &plan, None, false).unwrap_or_default(),
                );
                mcps.push(CapabilityStatusEntry {
                    name: definition.name.clone(),
                    kind: ResourceKind::Mcp,
                    source_scope: SourceScope::Global,
                    managed_by_arc: true,
                    declared_targets: definition.targets.clone(),
                    resolution: if is_fully_shadowed_by_project_targets(
                        &definition.name,
                        &targets,
                        &project_mcp_entries,
                    ) {
                        ResourceResolution::Shadowed
                    } else {
                        ResourceResolution::Active
                    },
                    targets,
                });
            }
        }
        Err(err) => mcps.push(invalid_global_capability_entry(
            ResourceKind::Mcp,
            "invalid-global-mcp-config",
            err.message,
        )),
    }
    mcps.extend(project_mcp_entries);

    let mut subagents = Vec::new();
    match load_global_subagents(paths) {
        Ok(global_subagents) => {
            for definition in global_subagents {
                let plan = SubagentApplyPlan {
                    prompt_path: PathBuf::from(&definition.prompt_file),
                    definition: definition.clone(),
                    source_scope: SourceScope::Global,
                };
                let targets = observe_capability_targets(
                    paths,
                    ResourceKind::SubAgent,
                    &definition.name,
                    SourceScope::Global,
                    None,
                    preview_subagent_plan(paths, cache, &plan, None).unwrap_or_default(),
                );
                subagents.push(CapabilityStatusEntry {
                    name: definition.name.clone(),
                    kind: ResourceKind::SubAgent,
                    source_scope: SourceScope::Global,
                    managed_by_arc: true,
                    declared_targets: definition.targets.clone(),
                    resolution: if is_fully_shadowed_by_project_targets(
                        &definition.name,
                        &targets,
                        &project_subagent_entries,
                    ) {
                        ResourceResolution::Shadowed
                    } else {
                        ResourceResolution::Active
                    },
                    targets,
                });
            }
        }
        Err(err) => subagents.push(invalid_global_capability_entry(
            ResourceKind::SubAgent,
            "invalid-global-subagent-config",
            err.message,
        )),
    }
    subagents.extend(project_subagent_entries);

    mcps.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.source_scope.cmp(&b.source_scope))
    });
    subagents.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.source_scope.cmp(&b.source_scope))
    });
    (mcps, subagents)
}

fn path_with_root(
    cwd: &Path,
    config: crate::project::ProjectConfig,
) -> Option<(PathBuf, crate::project::ProjectConfig)> {
    let config_path = find_project_config(cwd)?;
    let root = config_path.parent()?.to_path_buf();
    Some((root, config))
}

fn invalid_capability_targets(
    cache: &DetectCache,
    declared_targets: Option<&Vec<String>>,
    desired_scope: DesiredScope,
    reason: String,
) -> Vec<CapabilityTargetStatus> {
    let targets = resolve_declared_targets(cache, declared_targets);
    if targets.is_empty() {
        return vec![CapabilityTargetStatus {
            agent: "-".to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope,
            applied_scope: AppliedScope::None,
            reason: Some(reason),
        }];
    }

    targets
        .into_iter()
        .map(|agent| CapabilityTargetStatus {
            agent,
            status: CapabilityTargetState::Failed,
            desired_scope,
            applied_scope: AppliedScope::None,
            reason: Some(reason.clone()),
        })
        .collect()
}

fn collect_project_mcp_entries(
    paths: &ArcPaths,
    cache: &DetectCache,
    tracked: &[TrackedCapabilityInstall],
    project_root: &Path,
    cfg: &crate::project::ProjectConfig,
) -> Vec<CapabilityStatusEntry> {
    let mut entries = Vec::new();
    for definition in &cfg.mcps {
        let mut definition = definition.clone();
        let targets = match validate_mcp_definition(&mut definition, SourceScope::Project) {
            Ok(()) => {
                let plan = McpApplyPlan {
                    definition: definition.clone(),
                    source_scope: SourceScope::Project,
                };
                let preview = preview_mcp_plan(
                    paths,
                    cache,
                    tracked,
                    &plan,
                    Some(project_root),
                    tracked_has_global_fallback(tracked, project_root, &definition.name),
                )
                .unwrap_or_else(|err| {
                    invalid_capability_targets(
                        cache,
                        definition.targets.as_ref(),
                        DesiredScope::Project,
                        err.message,
                    )
                });
                observe_capability_targets(
                    paths,
                    ResourceKind::Mcp,
                    &definition.name,
                    SourceScope::Project,
                    Some(project_root),
                    preview,
                )
            }
            Err(err) => invalid_capability_targets(
                cache,
                definition.targets.as_ref(),
                DesiredScope::Project,
                err.message,
            ),
        };
        entries.push(CapabilityStatusEntry {
            name: definition.name.clone(),
            kind: ResourceKind::Mcp,
            source_scope: SourceScope::Project,
            managed_by_arc: true,
            declared_targets: definition.targets.clone(),
            resolution: ResourceResolution::Active,
            targets,
        });
    }
    entries
}

fn collect_project_subagent_entries(
    paths: &ArcPaths,
    cache: &DetectCache,
    project_root: &Path,
    cfg: &crate::project::ProjectConfig,
) -> Vec<CapabilityStatusEntry> {
    let mut entries = Vec::new();
    for definition in &cfg.subagents {
        let mut definition = definition.clone();
        let targets =
            match validate_subagent_definition(&mut definition, SourceScope::Project, project_root)
            {
                Ok(prompt_path) => {
                    let plan = SubagentApplyPlan {
                        prompt_path,
                        definition: definition.clone(),
                        source_scope: SourceScope::Project,
                    };
                    let preview = preview_subagent_plan(paths, cache, &plan, Some(project_root))
                        .unwrap_or_else(|err| {
                            invalid_capability_targets(
                                cache,
                                definition.targets.as_ref(),
                                DesiredScope::Project,
                                err.message,
                            )
                        });
                    observe_capability_targets(
                        paths,
                        ResourceKind::SubAgent,
                        &definition.name,
                        SourceScope::Project,
                        Some(project_root),
                        preview,
                    )
                }
                Err(err) => invalid_capability_targets(
                    cache,
                    definition.targets.as_ref(),
                    DesiredScope::Project,
                    err.message,
                ),
            };
        entries.push(CapabilityStatusEntry {
            name: definition.name.clone(),
            kind: ResourceKind::SubAgent,
            source_scope: SourceScope::Project,
            managed_by_arc: true,
            declared_targets: definition.targets.clone(),
            resolution: ResourceResolution::Active,
            targets,
        });
    }
    entries
}

fn tracked_has_global_fallback(
    tracked: &[TrackedCapabilityInstall],
    project_root: &Path,
    name: &str,
) -> bool {
    tracked.iter().any(|record| {
        record.kind == ResourceKind::Mcp
            && record.name == name
            && record.source_scope == SourceScope::Project
            && record.applied_scope == crate::agent::AppliedResourceScope::GlobalFallback
            && record.project_root.as_deref() == Some(project_root)
    })
}

fn observe_capability_targets(
    paths: &ArcPaths,
    kind: ResourceKind,
    name: &str,
    source_scope: SourceScope,
    project_root: Option<&Path>,
    targets: Vec<CapabilityTargetStatus>,
) -> Vec<CapabilityTargetStatus> {
    targets
        .into_iter()
        .map(|mut target| {
            let Some(record) =
                tracking_record_for_target(kind.clone(), name, source_scope, &target, project_root)
            else {
                return target;
            };
            match capability_install_present(paths, &record) {
                Ok(true) => target,
                Ok(false) => {
                    target.status = CapabilityTargetState::Failed;
                    target.reason = Some("drift_missing_from_disk".to_string());
                    target
                }
                Err(err) => {
                    target.status = CapabilityTargetState::Failed;
                    target.reason = Some(err.message);
                    target
                }
            }
        })
        .collect()
}

fn invalid_global_capability_entry(
    kind: ResourceKind,
    name: &str,
    reason: String,
) -> CapabilityStatusEntry {
    CapabilityStatusEntry {
        name: name.to_string(),
        kind,
        source_scope: SourceScope::Global,
        managed_by_arc: true,
        declared_targets: None,
        resolution: ResourceResolution::Active,
        targets: vec![CapabilityTargetStatus {
            agent: "-".to_string(),
            status: CapabilityTargetState::Failed,
            desired_scope: DesiredScope::Global,
            applied_scope: AppliedScope::None,
            reason: Some(reason),
        }],
    }
}

fn is_fully_shadowed_by_project_targets(
    name: &str,
    global_targets: &[CapabilityTargetStatus],
    project_entries: &[CapabilityStatusEntry],
) -> bool {
    let global_agents: BTreeSet<String> = global_targets
        .iter()
        .filter(|target| target.status == CapabilityTargetState::Applied)
        .map(|target| target.agent.clone())
        .collect();
    if global_agents.is_empty() {
        return false;
    }
    let project_agents: BTreeSet<String> = project_entries
        .iter()
        .filter(|entry| entry.name == name)
        .flat_map(|entry| entry.targets.iter())
        .filter(|target| target.status != CapabilityTargetState::Skipped)
        .map(|target| target.agent.clone())
        .collect();
    global_agents.is_subset(&project_agents)
}

fn collect_actions(
    project: &ProjectStatusSection,
    agents: &[AgentRuntimeStatus],
    mcps: &[CapabilityStatusEntry],
    subagents: &[CapabilityStatusEntry],
) -> Vec<RecommendedAction> {
    let mut actions = Vec::new();

    match project.state {
        ProjectState::None => {}
        ProjectState::Invalid => {
            actions.push(RecommendedAction {
                severity: ActionSeverity::Warn,
                message: "Fix arc.toml before relying on project status.".to_string(),
                command: None,
            });
        }
        ProjectState::Active => {
            if let Some(summary) = &project.summary {
                if summary.target_agents == 0 && summary.required_skills > 0 {
                    actions.push(RecommendedAction {
                        severity: ActionSeverity::Warn,
                        message: "Detect a project-capable agent to materialize required skills in the repo.".to_string(),
                        command: None,
                    });
                } else if summary.missing_skills > 0 || summary.partial_skills > 0 {
                    actions.push(RecommendedAction {
                        severity: ActionSeverity::Warn,
                        message:
                            "Materialize required skills for every detected project-capable agent."
                                .to_string(),
                        command: Some("arc project apply --all-agents".to_string()),
                    });
                }
                if summary.unavailable_skills > 0 {
                    actions.push(RecommendedAction {
                        severity: ActionSeverity::Warn,
                        message: "Some required skills are not available in the current catalogs."
                            .to_string(),
                        command: Some("arc skill list".to_string()),
                    });
                }
            }
            if let Some(provider) = &project.provider {
                for agent in &provider.agents {
                    match agent.state {
                        ProviderMatchState::Matched => {}
                        ProviderMatchState::Mismatch => actions.push(RecommendedAction {
                            severity: ActionSeverity::Warn,
                            message: format!(
                                "{} is not using project provider '{}'.",
                                agent.name, provider.name
                            ),
                            command: Some(format!(
                                "arc provider use {} --agent {}",
                                provider.name, agent.id
                            )),
                        }),
                        ProviderMatchState::MissingProfile => actions.push(RecommendedAction {
                            severity: ActionSeverity::Warn,
                            message: format!(
                                "{} does not have provider profile '{}'.",
                                agent.name, provider.name
                            ),
                            command: Some("arc provider list".to_string()),
                        }),
                    }
                }
            }
        }
    }

    if agents.is_empty() {
        actions.push(RecommendedAction {
            severity: ActionSeverity::Info,
            message: "Install a supported coding agent to get started.".to_string(),
            command: None,
        });
    }

    let project_capability_issue = mcps
        .iter()
        .chain(subagents.iter())
        .filter(|entry| entry.source_scope == SourceScope::Project)
        .flat_map(|entry| entry.targets.iter())
        .any(|target| target.status != CapabilityTargetState::Applied);
    if project_capability_issue {
        actions.push(RecommendedAction {
            severity: ActionSeverity::Warn,
            message: "Project MCP/subagent rollout has failures, drift, or unsupported targets."
                .to_string(),
            command: Some("arc project apply".to_string()),
        });
    }

    let global_capability_issue = mcps
        .iter()
        .chain(subagents.iter())
        .filter(|entry| entry.source_scope == SourceScope::Global)
        .flat_map(|entry| entry.targets.iter())
        .any(|target| target.status == CapabilityTargetState::Failed);
    if global_capability_issue {
        actions.push(RecommendedAction {
            severity: ActionSeverity::Warn,
            message: "Global MCP/subagent state has invalid or drifted installs.".to_string(),
            command: Some("arc status --format json".to_string()),
        });
    }

    actions
}

fn infer_project_name(cwd: &Path, config_path: Option<&Path>) -> String {
    config_path
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .or_else(|| cwd.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string())
}

fn count_skills_by_agent(items: &[InstalledResource]) -> BTreeMap<String, usize> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::detect::{AgentInfo, DetectCache};

    fn fake_agent(name: &str) -> AgentInfo {
        AgentInfo {
            name: name.to_string(),
            detected: true,
            root: Some(PathBuf::from(format!("/tmp/{name}"))),
            executable: Some(name.to_string()),
            version: Some("1.0.0".to_string()),
        }
    }

    #[test]
    fn collect_status_reports_missing_project_when_arc_toml_is_absent() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        let paths = ArcPaths::with_user_home(home.path());
        let cache = DetectCache::from_map(BTreeMap::new());

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        assert!(matches!(snapshot.project.state, ProjectState::None));
        assert!(snapshot.project.summary.is_none());
    }

    #[test]
    fn collect_status_reports_invalid_project_when_arc_toml_cannot_parse() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        fs::write(cwd.path().join("arc.toml"), "api_key = \"secret\"\n").unwrap();
        let paths = ArcPaths::with_user_home(home.path());
        let cache = DetectCache::from_map(BTreeMap::new());

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        assert!(matches!(snapshot.project.state, ProjectState::Invalid));
        assert!(snapshot.project.error.is_some());
    }

    #[test]
    fn collect_status_reports_project_skill_rollout_per_agent() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        fs::write(
            cwd.path().join("arc.toml"),
            "[skills]\nrequire = [\"my-skill\"]\n",
        )
        .unwrap();
        let skill_dir = home.path().join(".arc-cli").join("skills").join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# my-skill\n").unwrap();
        let codex_skill = cwd.path().join(".agents").join("skills").join("my-skill");
        fs::create_dir_all(codex_skill.parent().unwrap()).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&skill_dir, &codex_skill).unwrap();
        #[cfg(not(unix))]
        fs::create_dir_all(&codex_skill).unwrap();

        let paths = ArcPaths::with_user_home(home.path());
        let cache = DetectCache::from_map(BTreeMap::from([
            ("codex".to_string(), fake_agent("codex")),
            ("claude".to_string(), fake_agent("claude")),
        ]));

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        assert!(matches!(snapshot.project.state, ProjectState::Active));
        let summary = snapshot.project.summary.expect("summary");
        assert_eq!(summary.partial_skills, 1);
        assert_eq!(snapshot.project.agents.len(), 2);
        assert_eq!(
            snapshot.project.skills[0].ready_on_agents,
            vec!["codex".to_string()]
        );
        assert_eq!(
            snapshot.project.skills[0].missing_on_agents,
            vec!["claude".to_string()]
        );
    }

    #[test]
    fn collect_status_marks_project_subagent_with_missing_prompt_as_failed() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        fs::write(
            cwd.path().join("arc.toml"),
            "[[subagents]]\nname = \"reviewer\"\ntargets = [\"codex\"]\nprompt_file = \".arc/reviewer.md\"\n",
        )
        .unwrap();
        let paths = ArcPaths::with_user_home(home.path());
        let cache =
            DetectCache::from_map(BTreeMap::from([("codex".to_string(), fake_agent("codex"))]));

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        assert_eq!(snapshot.subagents.len(), 1);
        assert_eq!(snapshot.subagents[0].targets.len(), 1);
        assert_eq!(
            snapshot.subagents[0].targets[0].status,
            CapabilityTargetState::Failed
        );
        assert!(
            snapshot.subagents[0].targets[0]
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("prompt_file not found"))
        );
    }
}
