use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

mod actions;
mod agents;
mod capabilities;
mod catalog;
mod project_status;

use crate::agent::{McpScopeSupport, SubagentSupport, agent_spec, project_skill_path};
use crate::capability::{
    AppliedScope, CapabilityStatusEntry, CapabilityTargetState, CapabilityTargetStatus,
    DesiredScope, McpApplyPlan, ResourceResolution, SourceScope, SubagentApplyPlan,
    capability_install_present, load_global_subagents, preview_mcp_plan, preview_subagent_plan,
    resolve_declared_targets, tracking_record_for_target, validate_mcp_definition,
    validate_subagent_targets,
};
use crate::detect::DetectCache;
use crate::engine::{InstallEngine, InstalledResource};
use crate::market::sources::MarketSourceRegistry;
use crate::mcp_registry::load_user_registry_mcps;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::project::{
    find_project_config, load_project_config, resolve_project_capability_requirements,
};
use crate::provider::{load_providers_for_agent, read_active_provider, supports_provider_agent};
use crate::skill::SkillRegistry;

use actions::collect_actions;
use agents::{collect_agents, count_skills_by_agent};
use capabilities::collect_capabilities;
use catalog::collect_catalog;
use project_status::collect_project;

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
    let (mcps, subagents) = collect_capabilities(paths, cwd, cache);
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

    fn write_provider_profile(home: &Path, agent: &str, body: &str) {
        let providers_dir = home.join(".arc-cli").join("providers");
        fs::create_dir_all(&providers_dir).unwrap();
        fs::write(providers_dir.join(format!("{agent}.toml")), body).unwrap();
    }

    fn write_active_provider(home: &Path, body: &str) {
        let providers_dir = home.join(".arc-cli").join("providers");
        fs::create_dir_all(&providers_dir).unwrap();
        fs::write(providers_dir.join("active.toml"), body).unwrap();
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
        let codex_skill = cwd.path().join("codex").join("skills").join("my-skill");
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
    fn collect_status_marks_missing_project_subagent_reference_as_failed() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        fs::write(
            cwd.path().join("arc.toml"),
            "[subagents]\nrequire = [\"reviewer\"]\n",
        )
        .unwrap();
        let paths = ArcPaths::with_user_home(home.path());
        let cache =
            DetectCache::from_map(BTreeMap::from([("codex".to_string(), fake_agent("codex"))]));

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        let entry = snapshot
            .subagents
            .iter()
            .find(|entry| entry.name == "reviewer" && entry.source_scope == SourceScope::Project)
            .expect("project subagent entry");
        assert_eq!(entry.targets.len(), 1);
        assert_eq!(entry.targets[0].status, CapabilityTargetState::Failed);
        assert!(
            entry.targets[0]
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("not_in_catalog"))
        );
    }

    #[test]
    fn collect_status_reports_provider_alignment_and_actions() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        fs::write(cwd.path().join("arc.toml"), "[provider]\nname = \"work\"\n").unwrap();
        write_provider_profile(
            home.path(),
            "claude",
            r#"
[work]
display_name = "Work"
description = "Work profile"
"#,
        );
        write_active_provider(
            home.path(),
            r#"
[claude]
active = "official"
"#,
        );

        let paths = ArcPaths::with_user_home(home.path());
        let cache = DetectCache::from_map(BTreeMap::from([
            ("claude".to_string(), fake_agent("claude")),
            ("codex".to_string(), fake_agent("codex")),
        ]));

        let snapshot = collect_status(&paths, cwd.path(), &cache);
        let provider = snapshot.project.provider.expect("provider section");
        assert_eq!(provider.name, "work");
        assert_eq!(provider.matched_agents, 0);
        assert_eq!(provider.mismatched_agents, 1);
        assert_eq!(provider.missing_profiles, 1);
        assert!(
            provider
                .agents
                .iter()
                .any(|item| item.id == "claude"
                    && matches!(item.state, ProviderMatchState::Mismatch))
        );
        assert!(
            provider.agents.iter().any(|item| item.id == "codex"
                && matches!(item.state, ProviderMatchState::MissingProfile))
        );
        assert!(snapshot.actions.iter().any(|action| {
            action.command.as_deref() == Some("arc provider use work --agent claude")
        }));
        assert!(
            snapshot
                .actions
                .iter()
                .any(|action| action.command.as_deref() == Some("arc provider list"))
        );
    }

    #[test]
    fn collect_status_suggests_installing_supported_agent_when_none_detected() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        let paths = ArcPaths::with_user_home(home.path());
        let cache = DetectCache::from_map(BTreeMap::new());

        let snapshot = collect_status(&paths, cwd.path(), &cache);

        assert!(snapshot.actions.iter().any(|action| {
            action.message == "Install a supported coding agent to get started."
                && matches!(action.severity, ActionSeverity::Info)
        }));
    }
}
