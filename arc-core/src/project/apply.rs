use std::path::Path;

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
    EffectiveConfig, ProjectConfig, find_project_config, load_project_config,
    resolve_effective_config,
};

#[derive(Debug, Clone)]
pub struct ProjectApplyPlan {
    pub project_config: Option<ProjectConfig>,
    pub effective: EffectiveConfig,
    pub provider_to_switch: Option<String>,
    pub market_events: Vec<ProjectMarketEvent>,
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
pub struct ProjectApplyExecution {
    pub provider_switch: Option<ProjectProviderSwitch>,
    pub skill_results: Vec<ProjectSkillApplyItem>,
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
    let provider_to_switch = effective
        .provider_to_switch(paths)
        .map_err(|err| err.with_exit_code(1))?
        .map(str::to_string);

    Ok(ProjectApplyPlan {
        project_config,
        effective,
        provider_to_switch,
        market_events,
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

    Ok(ProjectApplyExecution {
        provider_switch,
        skill_results,
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

impl ProjectApplyExecution {
    pub fn has_issues(&self, effective: &EffectiveConfig) -> bool {
        !effective.missing_unavailable.is_empty()
            || self.skill_results.iter().any(|item| {
                matches!(
                    item.status,
                    ProjectSkillApplyStatus::NotFound | ProjectSkillApplyStatus::Failed { .. }
                )
            })
    }
}
