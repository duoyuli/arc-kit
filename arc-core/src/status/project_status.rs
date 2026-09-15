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
            installations: None,
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
                installations: None,
            };
        }
    };

    let root_ref = root.as_deref().expect("project configuration has a parent");
    let recorded =
        crate::project::skills::recorded_project_agents(paths, root_ref).unwrap_or_default();
    let options = crate::project::skills::ProjectSkillOptions {
        dry_run: true,
        all_agents: recorded.is_empty(),
        ..Default::default()
    };
    let installations = crate::project::skills::reconcile_project_skills(
        paths,
        cache,
        root_ref,
        &config.skills.require,
        &options,
        false,
        &[],
    );
    use crate::skill::install::{InstallAction, InstallStatus};
    let target_ids: std::collections::BTreeSet<_> = installations
        .items
        .iter()
        .map(|item| item.agent.clone())
        .filter(|name| !name.is_empty())
        .collect();
    let skills: Vec<ProjectSkillRollout> = config
        .skills
        .require
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|name| {
            let relevant: Vec<_> = installations
                .items
                .iter()
                .filter(|item| {
                    item.skill == *name
                        && !matches!(item.action, InstallAction::Remove | InstallAction::Forget)
                })
                .collect();
            let ready_on_agents: Vec<_> = relevant
                .iter()
                .filter(|item| item.status == InstallStatus::Kept)
                .map(|item| item.agent.clone())
                .collect();
            let missing_on_agents: Vec<_> = target_ids
                .iter()
                .filter(|agent| !ready_on_agents.contains(agent))
                .cloned()
                .collect();
            let state = if !installations.errors.is_empty() {
                ProjectSkillState::Conflict
            } else if relevant
                .iter()
                .any(|item| item.status == InstallStatus::Unmanaged)
            {
                ProjectSkillState::Unmanaged
            } else if relevant.iter().any(|item| {
                item.status == InstallStatus::Unavailable
                    || (item.status == InstallStatus::Unresolved
                        && item.action == InstallAction::Inspect
                        && item.source_path.is_none())
            }) {
                ProjectSkillState::Unavailable
            } else if relevant.iter().any(|item| {
                matches!(
                    item.status,
                    InstallStatus::Conflict
                        | InstallStatus::Unresolved
                        | InstallStatus::Failed
                        | InstallStatus::CleanupPending
                )
            }) {
                ProjectSkillState::Conflict
            } else if relevant
                .iter()
                .any(|item| item.action == InstallAction::Refresh)
            {
                ProjectSkillState::Outdated
            } else if !ready_on_agents.is_empty() && missing_on_agents.is_empty() {
                ProjectSkillState::Ready
            } else if !ready_on_agents.is_empty() {
                ProjectSkillState::Partial
            } else {
                ProjectSkillState::Missing
            };
            ProjectSkillRollout {
                name: name.clone(),
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
    let project_agents: Vec<ProjectTargetStatus> = target_ids
        .iter()
        .map(|agent| ProjectTargetStatus {
            id: agent.clone(),
            name: agent_spec(agent)
                .map(|spec| spec.display_name.to_string())
                .unwrap_or_else(|| agent.clone()),
            ready_skill_count: skills
                .iter()
                .filter(|skill| skill.ready_on_agents.contains(agent))
                .count(),
            total_available_skill_count: total_available_skills,
            provider_status: None,
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
        attention_skills: skills
            .iter()
            .filter(|skill| {
                matches!(
                    skill.state,
                    ProjectSkillState::Unmanaged
                        | ProjectSkillState::Conflict
                        | ProjectSkillState::Outdated
                )
            })
            .count(),
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
        installations: Some(installations),
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
            let has_profile =
                load_providers_for_agent(&providers_dir, &agent.id).is_ok_and(|providers| {
                    providers
                        .iter()
                        .any(|provider| provider.name == provider_name)
                });
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
