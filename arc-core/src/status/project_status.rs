use super::*;

pub(super) fn collect_project(
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

fn infer_project_name(cwd: &Path, config_path: Option<&Path>) -> String {
    config_path
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .or_else(|| cwd.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string())
}
