use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::models::ResourceKind;
use crate::paths::ArcPaths;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedResourceScope {
    #[serde(alias = "global_fallback")]
    Global,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpScopeSupport {
    ProjectNative,
    GlobalOnly,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpTransportSupport {
    pub supports_stdio: bool,
    pub supports_sse: bool,
    pub supports_streamable_http: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentSupport {
    Native,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentFormat {
    TomlDeveloperInstructions,
    MarkdownFrontmatter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigFormat {
    JsonMapTypedMcpServers,
    JsonMapPlainMcpServers,
    JsonMapGeminiMcpServers,
    JsonMapKimiMcpServers,
    JsonOpenCode,
    TomlCodexMcpServers,
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
    pub mcp_scope_support: McpScopeSupport,
    pub mcp_transport_support: McpTransportSupport,
    pub subagent_support: SubagentSupport,
    pub mcp_config_format: Option<McpConfigFormat>,
    pub mcp_global_config_parts: &'static [&'static str],
    pub mcp_project_config_parts: &'static [&'static str],
    pub subagent_global_dir_parts: &'static [&'static str],
    pub subagent_project_dir_parts: &'static [&'static str],
    pub subagent_format: Option<SubagentFormat>,
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
            mcp_scope_support: McpScopeSupport::ProjectNative,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: false,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Native,
            mcp_config_format: Some(McpConfigFormat::JsonMapTypedMcpServers),
            mcp_global_config_parts: &[".claude.json"],
            mcp_project_config_parts: &[".mcp.json"],
            subagent_global_dir_parts: &[".claude", "agents"],
            subagent_project_dir_parts: &[".claude", "agents"],
            subagent_format: Some(SubagentFormat::MarkdownFrontmatter),
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
            project_skills_parts: &["codex", "skills"],
            provider_kind: Some(ProviderKind::Codex),
            provider_seed: Some(include_str!("../provider/seed/codex.toml")),
            mcp_scope_support: McpScopeSupport::ProjectNative,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: false,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Native,
            mcp_config_format: Some(McpConfigFormat::TomlCodexMcpServers),
            mcp_global_config_parts: &[".codex", "config.toml"],
            mcp_project_config_parts: &[".codex", "config.toml"],
            subagent_global_dir_parts: &[".codex", "agents"],
            subagent_project_dir_parts: &[".codex", "agents"],
            subagent_format: Some(SubagentFormat::TomlDeveloperInstructions),
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
            mcp_scope_support: McpScopeSupport::Unsupported,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: false,
                supports_sse: false,
                supports_streamable_http: false,
            },
            subagent_support: SubagentSupport::Unsupported,
            mcp_config_format: None,
            mcp_global_config_parts: &[],
            mcp_project_config_parts: &[],
            subagent_global_dir_parts: &[],
            subagent_project_dir_parts: &[],
            subagent_format: None,
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
            mcp_scope_support: McpScopeSupport::ProjectNative,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: true,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Unsupported,
            mcp_config_format: Some(McpConfigFormat::JsonMapPlainMcpServers),
            mcp_global_config_parts: &[".cursor", "mcp.json"],
            mcp_project_config_parts: &[".cursor", "mcp.json"],
            subagent_global_dir_parts: &[],
            subagent_project_dir_parts: &[],
            subagent_format: None,
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
            mcp_scope_support: McpScopeSupport::ProjectNative,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: true,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Native,
            mcp_config_format: Some(McpConfigFormat::JsonOpenCode),
            mcp_global_config_parts: &[".config", "opencode", "opencode.json"],
            mcp_project_config_parts: &["opencode.json"],
            subagent_global_dir_parts: &[".config", "opencode", "agents"],
            subagent_project_dir_parts: &[".opencode", "agents"],
            subagent_format: Some(SubagentFormat::MarkdownFrontmatter),
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
            mcp_scope_support: McpScopeSupport::ProjectNative,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: true,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Unsupported,
            mcp_config_format: Some(McpConfigFormat::JsonMapGeminiMcpServers),
            mcp_global_config_parts: &[".gemini", "settings.json"],
            mcp_project_config_parts: &[".gemini", "settings.json"],
            subagent_global_dir_parts: &[],
            subagent_project_dir_parts: &[],
            subagent_format: None,
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
            mcp_scope_support: McpScopeSupport::GlobalOnly,
            mcp_transport_support: McpTransportSupport {
                supports_stdio: true,
                supports_sse: false,
                supports_streamable_http: true,
            },
            subagent_support: SubagentSupport::Unsupported,
            mcp_config_format: Some(McpConfigFormat::JsonMapKimiMcpServers),
            mcp_global_config_parts: &[".kimi", "mcp.json"],
            mcp_project_config_parts: &[],
            subagent_global_dir_parts: &[],
            subagent_project_dir_parts: &[],
            subagent_format: None,
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
            ResourceKind::Mcp => !matches!(spec.mcp_scope_support, McpScopeSupport::Unsupported),
            ResourceKind::SubAgent => spec.subagent_support == SubagentSupport::Native,
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

pub fn agent_mcp_path(
    paths: &ArcPaths,
    agent_id: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Option<PathBuf> {
    let spec = agent_spec(agent_id)?;
    let parts = match scope {
        AppliedResourceScope::Global => spec.mcp_global_config_parts,
        AppliedResourceScope::Project => spec.mcp_project_config_parts,
    };
    if parts.is_empty() {
        return None;
    }
    let mut path = match scope {
        AppliedResourceScope::Global => paths.user_home().to_path_buf(),
        AppliedResourceScope::Project => project_root?.to_path_buf(),
    };
    for part in parts {
        path = path.join(part);
    }
    Some(path)
}

pub fn agent_subagent_dir(
    paths: &ArcPaths,
    agent_id: &str,
    scope: AppliedResourceScope,
    project_root: Option<&Path>,
) -> Option<PathBuf> {
    let spec = agent_spec(agent_id)?;
    let parts = match scope {
        AppliedResourceScope::Global => spec.subagent_global_dir_parts,
        AppliedResourceScope::Project => spec.subagent_project_dir_parts,
    };
    if parts.is_empty() {
        return None;
    }
    let mut path = match scope {
        AppliedResourceScope::Global => paths.user_home().to_path_buf(),
        AppliedResourceScope::Project => project_root?.to_path_buf(),
    };
    for part in parts {
        path = path.join(part);
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_project_mcp_path_is_project_root_config() {
        let temp = tempfile::tempdir().unwrap();
        let paths = ArcPaths::with_user_home(temp.path().join("home"));
        let project = temp.path().join("repo");
        let path = agent_mcp_path(
            &paths,
            "opencode",
            AppliedResourceScope::Project,
            Some(&project),
        )
        .unwrap();
        assert_eq!(path, project.join("opencode.json"));
    }

    #[test]
    fn openclaw_has_no_mcp_support_in_arc() {
        let owl = agent_spec("openclaw").unwrap();
        assert!(matches!(
            owl.mcp_scope_support,
            McpScopeSupport::Unsupported
        ));
        assert!(owl.mcp_config_format.is_none());
        assert!(owl.mcp_global_config_parts.is_empty());
        assert!(owl.mcp_project_config_parts.is_empty());
    }
}
