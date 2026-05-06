use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

use crate::models::ResourceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillInstallStrategy {
    Symlink,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Claude,
    Codex,
}

#[derive(Debug, Clone)]
pub struct AgentSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub supports_skills: bool,
    pub skill_install_strategy: SkillInstallStrategy,
    pub executable: &'static str,
    pub version_flag: &'static str,
    pub home_parts: &'static [&'static str],
    pub skills_subdir: &'static str,
    pub supports_project_skills: bool,
    pub project_skills_parts: &'static [&'static str],
    pub provider_kind: Option<ProviderKind>,
    pub provider_seed: Option<&'static str>,
}

pub static AGENT_SPECS: Lazy<Vec<AgentSpec>> = Lazy::new(|| {
    vec![
        AgentSpec {
            id: "claude",
            display_name: "Claude Code",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "claude",
            version_flag: "-v",
            home_parts: &[".claude"],
            skills_subdir: "skills",
            supports_project_skills: true,
            project_skills_parts: &[".claude", "skills"],
            provider_kind: Some(ProviderKind::Claude),
            provider_seed: Some(include_str!("../provider/seed/claude.toml")),
        },
        AgentSpec {
            id: "codex",
            display_name: "Codex",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "codex",
            version_flag: "-V",
            home_parts: &[".codex"],
            skills_subdir: "skills",
            supports_project_skills: true,
            project_skills_parts: &[".codex", "skills"],
            provider_kind: Some(ProviderKind::Codex),
            provider_seed: Some(include_str!("../provider/seed/codex.toml")),
        },
        AgentSpec {
            id: "openclaw",
            display_name: "OpenClaw",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Copy,
            executable: "openclaw",
            version_flag: "-v",
            home_parts: &[".openclaw"],
            skills_subdir: "skills",
            supports_project_skills: false,
            project_skills_parts: &[],
            provider_kind: None,
            provider_seed: None,
        },
        AgentSpec {
            id: "cursor",
            display_name: "Cursor CLI",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "agent",
            version_flag: "-v",
            home_parts: &[".cursor"],
            skills_subdir: "skills-cursor",
            supports_project_skills: true,
            project_skills_parts: &[".cursor", "skills"],
            provider_kind: None,
            provider_seed: None,
        },
        AgentSpec {
            id: "opencode",
            display_name: "OpenCode",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "opencode",
            version_flag: "-v",
            home_parts: &[".config", "opencode"],
            skills_subdir: "skills",
            supports_project_skills: true,
            project_skills_parts: &[".opencode", "skills"],
            provider_kind: None,
            provider_seed: None,
        },
        AgentSpec {
            id: "gemini",
            display_name: "Gemini CLI",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "gemini",
            version_flag: "-v",
            home_parts: &[".gemini"],
            skills_subdir: "skills",
            supports_project_skills: true,
            project_skills_parts: &[".gemini", "skills"],
            provider_kind: None,
            provider_seed: None,
        },
        AgentSpec {
            id: "kimi",
            display_name: "Kimi CLI",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "kimi",
            version_flag: "--version",
            home_parts: &[".kimi"],
            skills_subdir: "skills",
            supports_project_skills: true,
            project_skills_parts: &[".kimi", "skills"],
            provider_kind: None,
            provider_seed: None,
        },
    ]
});

pub fn agent_specs() -> &'static [AgentSpec] {
    AGENT_SPECS.as_slice()
}

pub fn agent_spec(id: &str) -> Option<&'static AgentSpec> {
    AGENT_SPECS.iter().find(|spec| spec.id == id)
}

pub fn ordered_agent_ids_for_resource_kind(kind: &ResourceKind) -> Vec<String> {
    AGENT_SPECS
        .iter()
        .filter(|spec| match kind {
            ResourceKind::Skill => spec.supports_skills,
            ResourceKind::ProviderProfile => false,
        })
        .map(|spec| spec.id.to_string())
        .collect()
}

pub fn default_install_targets(kind: &ResourceKind) -> Vec<String> {
    ordered_agent_ids_for_resource_kind(kind)
}

pub fn resource_install_subdir(kind: &ResourceKind, agent_id: &str) -> String {
    if matches!(kind, ResourceKind::Skill)
        && let Some(spec) = agent_spec(agent_id)
    {
        return spec.skills_subdir.to_string();
    }
    kind.as_str().to_string()
}

pub fn project_skill_path(
    project_root: &Path,
    agent_id: &str,
    skill_name: &str,
) -> Option<PathBuf> {
    let spec = agent_spec(agent_id)?;
    if !spec.supports_project_skills {
        return None;
    }
    let mut path = project_root.to_path_buf();
    for part in spec.project_skills_parts {
        path = path.join(part);
    }
    Some(path.join(skill_name))
}
