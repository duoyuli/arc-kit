use std::path::Path;

use super::skills::{ProjectSkillOptions, reconcile_project_skills};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::market::bootstrap::sync_market_source_resources;
use crate::market::sources::MarketSourceRegistry;
use crate::paths::ArcPaths;
use crate::provider::{apply_provider, load_providers_for_agent, supported_provider_agents};
use crate::skill::SkillRegistry;
use crate::skill::install::{InstallReport, InstallStatus};

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
    pub preparation_errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectMarketEvent {
    pub source_id: String,
    pub url: String,
    pub status: ProjectMarketEventStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectMarketEventStatus {
    Added,
    Failed,
    Planned,
}

#[derive(Debug, Clone, serde::Serialize)]
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
    pub installs: InstallReport,
}

pub fn prepare_project_apply(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
) -> Result<ProjectApplyPlan> {
    prepare_project_apply_with_mode(paths, cache, cwd, false)
}

pub fn prepare_project_apply_with_mode(
    paths: &ArcPaths,
    cache: &DetectCache,
    cwd: &Path,
    dry_run: bool,
) -> Result<ProjectApplyPlan> {
    let config_path = find_project_config(cwd)
        .ok_or_else(|| ArcError::new("No arc.toml found in current directory or its parents."))?;
    let project_config = Some(load_project_config(&config_path)?);
    let mut preparation_errors = Vec::new();
    let market_events = if dry_run {
        let registry = MarketSourceRegistry::new(paths.clone());
        let sources = registry.load();
        project_config
            .as_ref()
            .unwrap()
            .markets
            .iter()
            .filter_map(|market| {
                let source_id = registry.generate_slug(&market.url);
                (!sources.contains_key(&source_id)).then(|| ProjectMarketEvent {
                    source_id,
                    url: market.url.clone(),
                    status: ProjectMarketEventStatus::Planned,
                })
            })
            .collect()
    } else {
        match sync_project_markets(paths, project_config.as_ref().unwrap()) {
            Ok(events) => {
                for event in &events {
                    if event.status == ProjectMarketEventStatus::Failed {
                        preparation_errors
                            .push(format!("failed to prepare project market {}", event.url));
                    }
                }
                events
            }
            Err(err) => {
                preparation_errors.push(err.message);
                Vec::new()
            }
        }
    };

    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    if !dry_run && let Err(err) = registry.bootstrap_catalog() {
        preparation_errors.push(err.message);
    }

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
        preparation_errors,
    })
}

pub fn execute_project_apply(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &ProjectApplyPlan,
    skill_targets: &[String],
) -> Result<ProjectApplyExecution> {
    execute_project_apply_with_options(
        paths,
        cache,
        plan,
        &ProjectSkillOptions {
            agents: skill_targets.to_vec(),
            ..Default::default()
        },
    )
}

pub fn execute_project_apply_with_options(
    paths: &ArcPaths,
    cache: &DetectCache,
    plan: &ProjectApplyPlan,
    options: &ProjectSkillOptions,
) -> Result<ProjectApplyExecution> {
    let root = plan
        .effective
        .project_root
        .as_ref()
        .ok_or_else(|| ArcError::new("project root missing"))?;
    let mut installs = reconcile_project_skills(
        paths,
        cache,
        root,
        &plan.effective.required_skills,
        options,
        false,
        &plan.preparation_errors,
    );
    let provider_switch = if installs.ok() && !options.dry_run {
        match plan
            .provider_to_switch
            .as_deref()
            .map(|name| apply_provider_switch(paths, name))
            .transpose()
        {
            Ok(provider) => provider,
            Err(err) => {
                installs
                    .errors
                    .push(format!("provider switch failed: {}", err.message));
                None
            }
        }
    } else {
        None
    };
    let skill_results = installs
        .items
        .iter()
        .filter_map(|item| {
            let status = match item.status {
                InstallStatus::Installed | InstallStatus::Refreshed | InstallStatus::Adopted => {
                    ProjectSkillApplyStatus::Installed {
                        agents: vec![item.agent.clone()],
                    }
                }
                status if status.is_issue() => ProjectSkillApplyStatus::Failed {
                    message: item
                        .message
                        .clone()
                        .unwrap_or_else(|| format!("{:?}", item.status)),
                },
                _ => return None,
            };
            Some(ProjectSkillApplyItem {
                name: item.skill.clone(),
                status,
            })
        })
        .collect();

    Ok(ProjectApplyExecution {
        provider_switch,
        skill_results,
        installs,
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
        let providers = load_providers_for_agent(&providers_dir, agent)?;
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

impl ProjectApplyExecution {
    pub fn has_issues(&self, effective: &EffectiveConfig) -> bool {
        !self.installs.ok()
            || !effective.missing_unavailable.is_empty()
            || self.skill_results.iter().any(|item| {
                matches!(
                    item.status,
                    ProjectSkillApplyStatus::NotFound | ProjectSkillApplyStatus::Failed { .. }
                )
            })
    }
}
