use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::detect::{DetectCache, coding_agent_spec, project_skill_path};
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
    let actions = collect_actions(&project, &agents);

    StatusSnapshot {
        project,
        agents,
        catalog,
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
            let spec = coding_agent_spec(agent_id);
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

fn collect_actions(
    project: &ProjectStatusSection,
    agents: &[AgentRuntimeStatus],
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
}
