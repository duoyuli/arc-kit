use std::path::Path;

use crate::capability::{
    apply_mcp_plan, apply_subagent_plan, list_tracked_capability_installs,
    remove_tracked_capability, tracking_record_for_target, validate_mcp_definition,
    validate_subagent_targets, CapabilityTargetState, CapabilityTargetStatus, McpApplyPlan,
    SourceScope, SubagentApplyPlan, TrackedCapabilityInstall,
};
use crate::detect::DetectCache;
use crate::engine::InstallEngine;
use crate::error::{ArcError, Result};
use crate::market::bootstrap::sync_market_source_resources;
use crate::market::sources::MarketSourceRegistry;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::provider::{apply_provider, load_providers_for_agent, supported_provider_agents};
use crate::skill::SkillRegistry;

use super::{
    find_project_config, load_project_config, resolve_effective_config,
    resolve_project_capability_requirements, EffectiveConfig, ProjectCapabilityRequirements,
    ProjectConfig,
};

#[derive(Debug, Clone)]
pub struct ProjectApplyPlan {
    pub project_config: Option<ProjectConfig>,
    pub effective: EffectiveConfig,
    pub provider_to_switch: Option<String>,
    pub market_events: Vec<ProjectMarketEvent>,
    pub capability_requirements: ProjectCapabilityRequirements,
}

#[derive(Debug, Clone)]
pub struct ProjectMarketEvent {
    pub source_id: String,
    pub url: String,
    pub status: ProjectMarketEventStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectMarketEventStatus {
    Added,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ProjectProviderSwitch {
    pub name: String,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ProjectSkillApplyStatus {
    Installed { agents: Vec<String> },
    NotFound,
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub struct ProjectSkillApplyItem {
    pub name: String,
    pub status: ProjectSkillApplyStatus,
}

#[derive(Debug, Clone)]
pub struct ProjectCapabilityApplyItem {
    pub kind: ResourceKind,
    pub name: String,
    pub status: CapabilityTargetStatus,
}

#[derive(Debug, Clone)]
pub struct ProjectApplyExecution {
    pub provider_switch: Option<ProjectProviderSwitch>,
    pub skill_results: Vec<ProjectSkillApplyItem>,
    pub capability_results: Vec<ProjectCapabilityApplyItem>,
    pub removed_capabilities: Vec<TrackedCapabilityInstall>,
}

pub fn prepare_project_apply(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<ProjectApplyPlan> {
    let project_config = if let Some(config_path) = find_project_config(cwd) {
        load_project_config(&config_path).ok()
    } else {
        None
    };

    let market_events = if let Some(cfg) = &project_config {
        sync_project_markets(paths, cfg)?
    } else {
        Vec::new()
    };

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    registry
        .bootstrap_catalog()
        .map_err(|err| err.with_exit_code(1))?;

    let effective = resolve_effective_config(paths, cwd, cache, &registry)
        .map_err(|err| err.with_exit_code(1))?;
    let capability_requirements = project_config
        .as_ref()
        .map(|cfg| resolve_project_capability_requirements(paths, cfg))
        .transpose()
        .map_err(|err| err.with_exit_code(1))?
        .unwrap_or_default();
    let provider_to_switch = effective
        .provider_to_switch(paths)
        .map_err(|err| err.with_exit_code(1))?
        .map(str::to_string);

    Ok(ProjectApplyPlan {
        project_config,
        effective,
        provider_to_switch,
        market_events,
        capability_requirements,
    })
}

pub fn execute_project_apply(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &ProjectApplyPlan,
    skill_targets: &[String],
) -> Result<ProjectApplyExecution> {
    let provider_switch = match plan.provider_to_switch.as_deref() {
        Some(provider_name) => Some(apply_provider_switch(paths, provider_name)?),
        None => None,
    };

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let skill_results = if plan.effective.missing_installable.is_empty() {
        Vec::new()
    } else {
        let project_root = plan.effective.project_root.as_ref().ok_or_else(|| {
            ArcError::new("internal error: arc.toml present but project root missing")
        })?;
        apply_project_skills(
            cache,
            &registry,
            &plan.effective,
            project_root,
            skill_targets,
        )?
    };

    let (capability_results, removed_capabilities) = if plan.project_config.is_some() {
        let project_root = plan.effective.project_root.as_ref().ok_or_else(|| {
            ArcError::new("internal error: arc.toml present but project root missing")
        })?;
        apply_project_capabilities(paths, cache, &plan.capability_requirements, project_root)?
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(ProjectApplyExecution {
        provider_switch,
        skill_results,
        capability_results,
        removed_capabilities,
    })
}

fn sync_project_markets(paths: &ArcPaths, cfg: &ProjectConfig) -> Result<Vec<ProjectMarketEvent>> {
    let market_registry = MarketSourceRegistry::new(paths.clone());
    let existing = market_registry.load();
    let mut events = Vec::new();

    for entry in &cfg.markets {
        let url = &entry.url;
        let source_id = market_registry.generate_slug(url);
        if existing.contains_key(&source_id) {
            continue;
        }
        match market_registry.add(url, "auto") {
            Ok(source) => {
                sync_market_source_resources(paths, &source)
                    .map_err(|err| err.with_exit_code(1))?;
                events.push(ProjectMarketEvent {
                    source_id,
                    url: url.clone(),
                    status: ProjectMarketEventStatus::Added,
                });
            }
            Err(_) => {
                events.push(ProjectMarketEvent {
                    source_id,
                    url: url.clone(),
                    status: ProjectMarketEventStatus::Failed,
                });
            }
        }
    }

    Ok(events)
}

fn apply_provider_switch(paths: &ArcPaths, provider_name: &str) -> Result<ProjectProviderSwitch> {
    let providers_dir = paths.providers_dir();
    let mut agents = Vec::new();
    for agent in supported_provider_agents() {
        let providers = load_providers_for_agent(&providers_dir, agent);
        if let Some(provider) = providers
            .into_iter()
            .find(|item| item.name == provider_name)
        {
            apply_provider(paths, &provider)?;
            agents.push(agent.to_string());
        }
    }
    Ok(ProjectProviderSwitch {
        name: provider_name.to_string(),
        agents,
    })
}

fn apply_project_skills(
    cache: &DetectCache,
    registry: &SkillRegistry,
    effective: &EffectiveConfig,
    project_root: &Path,
    skill_targets: &[String],
) -> Result<Vec<ProjectSkillApplyItem>> {
    if effective.missing_installable.is_empty() {
        return Ok(Vec::new());
    }
    if skill_targets.is_empty() {
        return Err(ArcError::new(
            "project skill targets are required when skills need installation",
        ));
    }

    let engine = InstallEngine::new(cache.clone());
    let mut results = Vec::new();
    for name in &effective.missing_installable {
        let Some(skill) = registry.find(name) else {
            results.push(ProjectSkillApplyItem {
                name: name.clone(),
                status: ProjectSkillApplyStatus::NotFound,
            });
            continue;
        };
        let source_path = match registry.resolve_source_path(&skill) {
            Ok(path) => path,
            Err(err) => {
                results.push(ProjectSkillApplyItem {
                    name: name.clone(),
                    status: ProjectSkillApplyStatus::Failed {
                        message: err.message,
                    },
                });
                continue;
            }
        };
        match engine.install_named_project(
            name,
            &ResourceKind::Skill,
            &source_path,
            project_root,
            skill_targets,
        ) {
            Ok(agents) => results.push(ProjectSkillApplyItem {
                name: name.clone(),
                status: ProjectSkillApplyStatus::Installed { agents },
            }),
            Err(err) => results.push(ProjectSkillApplyItem {
                name: name.clone(),
                status: ProjectSkillApplyStatus::Failed {
                    message: err.message,
                },
            }),
        }
    }
    Ok(results)
}

fn apply_project_capabilities(
    paths: &ArcPaths,
    cache: &DetectCache,
    requirements: &ProjectCapabilityRequirements,
    project_root: &Path,
) -> Result<(
    Vec<ProjectCapabilityApplyItem>,
    Vec<TrackedCapabilityInstall>,
)> {
    let mut items = Vec::new();

    for definition in &requirements.mcps {
        let mut definition = definition.clone();
        validate_mcp_definition(&mut definition)?;
        let statuses = apply_mcp_plan(
            paths,
            cache,
            &McpApplyPlan {
                definition: definition.clone(),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        items.extend(
            statuses
                .into_iter()
                .map(|status| ProjectCapabilityApplyItem {
                    kind: ResourceKind::Mcp,
                    name: definition.name.clone(),
                    status,
                }),
        );
    }

    for entry in &requirements.subagents {
        let definition = entry.definition.clone();
        let prompt_body = entry.prompt_body.clone();
        let _ = validate_subagent_targets(cache, &definition)?;
        let statuses = apply_subagent_plan(
            paths,
            cache,
            &SubagentApplyPlan {
                definition: definition.clone(),
                prompt_path: None,
                prompt_body: Some(prompt_body),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        items.extend(
            statuses
                .into_iter()
                .map(|status| ProjectCapabilityApplyItem {
                    kind: ResourceKind::SubAgent,
                    name: definition.name.clone(),
                    status,
                }),
        );
    }

    let removed_capabilities =
        cleanup_removed_project_capabilities(paths, cache, requirements, project_root)?;

    Ok((items, removed_capabilities))
}

fn cleanup_removed_project_capabilities(
    paths: &ArcPaths,
    cache: &DetectCache,
    requirements: &ProjectCapabilityRequirements,
    project_root: &Path,
) -> Result<Vec<TrackedCapabilityInstall>> {
    let desired_records =
        desired_project_capability_records(paths, cache, requirements, project_root)?;
    let tracked = list_tracked_capability_installs(paths);
    let mut removed_items = Vec::new();
    for record in tracked.into_iter().filter(|record| {
        record.source_scope == SourceScope::Project
            && record.project_root.as_deref() == Some(project_root)
            && matches!(record.kind, ResourceKind::Mcp | ResourceKind::SubAgent)
            && !desired_records.iter().any(|desired| desired == record)
    }) {
        remove_tracked_capability(paths, &record, Some(project_root))?;
        removed_items.push(record);
    }
    Ok(removed_items)
}

fn desired_project_capability_records(
    paths: &ArcPaths,
    cache: &DetectCache,
    requirements: &ProjectCapabilityRequirements,
    project_root: &Path,
) -> Result<Vec<TrackedCapabilityInstall>> {
    let mut desired = Vec::new();

    for definition in &requirements.mcps {
        let mut definition = definition.clone();
        validate_mcp_definition(&mut definition)?;
        let statuses = crate::capability::preview_mcp_plan(
            paths,
            cache,
            &McpApplyPlan {
                definition: definition.clone(),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        desired.extend(statuses.into_iter().filter_map(|status| {
            tracking_record_for_target(
                ResourceKind::Mcp,
                &definition.name,
                SourceScope::Project,
                &status,
                Some(project_root),
            )
        }));
    }

    for entry in &requirements.subagents {
        let definition = entry.definition.clone();
        let _ = validate_subagent_targets(cache, &definition)?;
        let statuses = crate::capability::preview_subagent_plan(
            paths,
            cache,
            &SubagentApplyPlan {
                definition: definition.clone(),
                prompt_path: None,
                prompt_body: Some(entry.prompt_body.clone()),
                source_scope: SourceScope::Project,
            },
            Some(project_root),
        )?;
        desired.extend(statuses.into_iter().filter_map(|status| {
            tracking_record_for_target(
                ResourceKind::SubAgent,
                &definition.name,
                SourceScope::Project,
                &status,
                Some(project_root),
            )
        }));
    }

    Ok(desired)
}

impl ProjectApplyExecution {
    pub fn has_issues(&self, effective: &EffectiveConfig) -> bool {
        !effective.missing_unavailable.is_empty()
            || !effective.missing_mcps_unavailable.is_empty()
            || !effective.missing_subagents_unavailable.is_empty()
            || self.skill_results.iter().any(|item| {
                matches!(
                    item.status,
                    ProjectSkillApplyStatus::NotFound | ProjectSkillApplyStatus::Failed { .. }
                )
            })
            || self
                .capability_results
                .iter()
                .any(|item| item.status.status != CapabilityTargetState::Applied)
    }
}
