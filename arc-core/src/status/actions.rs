use super::*;

pub(super) fn collect_actions(
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
                        message:
                            "Detect a project-capable agent to materialize required skills in the repo."
                                .to_string(),
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
