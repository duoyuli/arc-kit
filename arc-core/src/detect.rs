use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub name: String,
    pub detected: bool,
    pub root: Option<PathBuf>,
    pub executable: Option<String>,
    pub version: Option<String>,
}

/// Cached agent detection results. Created once at CLI entry, passed to all subsystems.
#[derive(Debug, Clone)]
pub struct DetectCache {
    agents: BTreeMap<String, AgentInfo>,
}

impl DetectCache {
    /// Detect all agents in parallel and cache results.
    pub fn new(paths: &ArcPaths) -> Self {
        Self {
            agents: parallel_detect_all(paths),
        }
    }

    /// Create from pre-built data (for testing).
    pub fn from_map(agents: BTreeMap<String, AgentInfo>) -> Self {
        Self { agents }
    }

    pub fn detected_agents(&self) -> &BTreeMap<String, AgentInfo> {
        &self.agents
    }

    pub fn get_agent(&self, name: &str) -> Option<&AgentInfo> {
        self.agents.get(name)
    }

    pub fn agents_for_install(&self, kind: &ResourceKind) -> Vec<String> {
        let ordered = ordered_agent_ids_for_resource_kind(kind);
        let picked: Vec<_> = ordered
            .iter()
            .filter(|id| self.agents.contains_key(id.as_str()))
            .cloned()
            .collect();
        if picked.is_empty() {
            default_install_targets(kind)
        } else {
            picked
        }
    }

    /// Like [`Self::agents_for_install`], but only agents that support project-local skill dirs
    /// (`CodingAgentSpec.supports_project_skills`). Used by `arc project apply` and project status.
    pub fn agents_for_project_skill_install(&self, kind: &ResourceKind) -> Vec<String> {
        self.agents_for_install(kind)
            .into_iter()
            .filter(|id| {
                coding_agent_spec(id)
                    .map(|s| s.supports_project_skills)
                    .unwrap_or(false)
            })
            .collect()
    }
}

fn parallel_detect_all(paths: &ArcPaths) -> BTreeMap<String, AgentInfo> {
    std::thread::scope(|s| {
        let handles: Vec<_> = CODING_AGENTS
            .iter()
            .map(|spec| s.spawn(|| (spec.id.to_string(), detect_from_spec(paths, spec))))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("agent detection panicked"))
            .filter(|(_, info)| info.detected)
            .collect()
    })
}

#[derive(Debug, Clone)]
/// Static configuration for one supported coding agent (executable, home layout, skills path, provider).
pub struct CodingAgentSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub supports_skills: bool,
    pub skill_install_strategy: SkillInstallStrategy,
    pub executable: &'static str,
    pub version_flag: &'static str,
    /// Path parts relative to user home, e.g. `&[".claude"]` → `~/.claude`
    pub home_parts: &'static [&'static str],
    pub skills_subdir: &'static str,
    /// When true, `arc project apply` may install required skills under the repo for this agent.
    /// When false (e.g. OpenClaw), project-local skill paths are not used; use global `arc skill install` only.
    pub supports_project_skills: bool,
    /// Path segments under the **project root** (repo) where this agent loads skills, e.g.
    /// `&[".claude", "skills"]` → `<repo>/.claude/skills/<name>/`. Ignored when `supports_project_skills` is false.
    pub project_skills_parts: &'static [&'static str],
    pub provider_kind: Option<ProviderKind>,
    pub provider_seed: Option<&'static str>,
}

pub static CODING_AGENTS: Lazy<Vec<CodingAgentSpec>> = Lazy::new(|| {
    vec![
        CodingAgentSpec {
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
            provider_seed: Some(include_str!("provider/seed/claude.toml")),
        },
        CodingAgentSpec {
            id: "codex",
            display_name: "Codex",
            supports_skills: true,
            skill_install_strategy: SkillInstallStrategy::Symlink,
            executable: "codex",
            version_flag: "-V",
            home_parts: &[".codex"],
            skills_subdir: "skills",
            supports_project_skills: true,
            // Codex CLI project-local skills (see OpenAI Codex docs / .agents/skills)
            project_skills_parts: &[".agents", "skills"],
            provider_kind: Some(ProviderKind::Codex),
            provider_seed: Some(include_str!("provider/seed/codex.toml")),
        },
        CodingAgentSpec {
            id: "openclaw",
            display_name: "OpenClaw",
            supports_skills: true,
            // Only OpenClaw uses directory copy for skills; all other agents use symlinks.
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
        CodingAgentSpec {
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
        CodingAgentSpec {
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
        CodingAgentSpec {
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
        CodingAgentSpec {
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

/// Alias for [`CodingAgentSpec`] (per-agent table row).
pub type AgentConfig = CodingAgentSpec;

/// Directory segment under an agent's home where resources of `kind` are installed.
///
/// For [`ResourceKind::Skill`], uses the agent's `skills_subdir` from [`coding_agent_spec`];
/// for other kinds, uses [`ResourceKind::as_str`].
pub fn resource_install_subdir(kind: &ResourceKind, agent_name: &str) -> String {
    if matches!(kind, ResourceKind::Skill)
        && let Some(spec) = coding_agent_spec(agent_name)
    {
        return spec.skills_subdir.to_string();
    }
    kind.as_str().to_string()
}

pub fn detect_agent(paths: &ArcPaths, name: &str) -> Result<AgentInfo, String> {
    let spec = CODING_AGENTS
        .iter()
        .find(|spec| spec.id == name)
        .ok_or_else(|| format!("unknown agent: {name}"))?;
    Ok(detect_from_spec(paths, spec))
}

pub fn detect_all_agents(paths: &ArcPaths) -> BTreeMap<String, AgentInfo> {
    CODING_AGENTS
        .iter()
        .map(|spec| (spec.id.to_string(), detect_from_spec(paths, spec)))
        .filter(|(_, info)| info.detected)
        .collect()
}

pub fn get_detected_agents(paths: &ArcPaths) -> Vec<String> {
    detect_all_agents(paths).into_keys().collect()
}

pub fn detect_agents_for_install(paths: &ArcPaths, kind: &ResourceKind) -> Vec<String> {
    let ordered = ordered_agent_ids_for_resource_kind(kind);
    let detected = detect_all_agents(paths);
    let picked: Vec<_> = ordered
        .iter()
        .filter(|agent_id| detected.contains_key(agent_id.as_str()))
        .cloned()
        .collect();
    if picked.is_empty() {
        default_install_targets(kind)
    } else {
        picked
    }
}

pub fn ordered_agent_ids_for_resource_kind(kind: &ResourceKind) -> Vec<String> {
    CODING_AGENTS
        .iter()
        .filter(|spec| matches!(kind, ResourceKind::Skill) && spec.supports_skills)
        .map(|spec| spec.id.to_string())
        .collect()
}

pub fn default_install_targets(kind: &ResourceKind) -> Vec<String> {
    ordered_agent_ids_for_resource_kind(kind)
}

pub fn coding_agent_spec(id: &str) -> Option<&'static CodingAgentSpec> {
    CODING_AGENTS.iter().find(|spec| spec.id == id)
}

/// Returns `<project_root>/<...project_skills_parts>/<skill_name>/` for this agent's project-local
/// skill layout (e.g. `<repo>/.claude/skills/<name>`).
pub fn project_skill_path(
    project_root: &Path,
    agent_id: &str,
    skill_name: &str,
) -> Option<PathBuf> {
    let spec = coding_agent_spec(agent_id)?;
    if !spec.supports_project_skills {
        return None;
    }
    let mut p = project_root.to_path_buf();
    for part in spec.project_skills_parts {
        p = p.join(part);
    }
    Some(p.join(skill_name))
}

/// True if `skill_name` is present under the project tree for every install target agent.
pub fn project_skills_satisfied_for_requirements(
    cache: &DetectCache,
    project_root: &Path,
    skill_name: &str,
) -> bool {
    let targets = cache.agents_for_project_skill_install(&ResourceKind::Skill);
    if targets.is_empty() {
        return false;
    }
    targets.iter().all(|agent_id| {
        project_skill_path(project_root, agent_id, skill_name)
            .map(|p| p.exists())
            .unwrap_or(false)
    })
}

fn detect_from_spec(paths: &ArcPaths, spec: &CodingAgentSpec) -> AgentInfo {
    let (executable, version) = detect_executable(spec);
    let detected = executable.is_some();
    let root = detected.then(|| paths.user_home().join(spec.home_parts.join("/")));
    AgentInfo {
        name: spec.id.to_string(),
        detected,
        root,
        executable,
        version,
    }
}

fn detect_executable(spec: &CodingAgentSpec) -> (Option<String>, Option<String>) {
    let Some(path) = resolve_executable_path(spec.executable) else {
        return (None, None);
    };
    let version =
        command_output_with_timeout(spec.executable, spec.version_flag, Duration::from_secs(2))
            .and_then(|out| {
                let raw = String::from_utf8_lossy(&out.stdout);
                extract_version(raw.trim())
            });
    (Some(path), version)
}

fn resolve_executable_path(name: &str) -> Option<String> {
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
}

fn command_output_with_timeout(executable: &str, arg: &str, timeout: Duration) -> Option<Output> {
    let mut child = Command::new(executable)
        .arg(arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .ok()
                    .filter(|out| out.status.success());
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Extract a version string from CLI output like "2.1.84 (Claude Code)" or "codex-cli 0.116.0".
pub fn extract_version(output: &str) -> Option<String> {
    let line = output.lines().next().unwrap_or("").trim();
    line.split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()) && t.contains('.'))
        .map(|t| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_version_claude() {
        assert_eq!(
            extract_version("2.1.84 (Claude Code)"),
            Some("2.1.84".to_string())
        );
    }

    #[test]
    fn extract_version_codex() {
        assert_eq!(
            extract_version("codex-cli 0.116.0"),
            Some("0.116.0".to_string())
        );
    }

    #[test]
    fn extract_version_plain() {
        assert_eq!(extract_version("0.35.1"), Some("0.35.1".to_string()));
    }

    #[test]
    fn extract_version_date_style() {
        assert_eq!(
            extract_version("2026.03.25-933d5a6"),
            Some("2026.03.25-933d5a6".to_string())
        );
    }

    #[test]
    fn extract_version_empty() {
        assert_eq!(extract_version(""), None);
        assert_eq!(extract_version("no version here"), None);
    }

    #[test]
    fn command_output_with_timeout_returns_none_for_hanging_command() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("hang.sh");
        std::fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();
        }

        let output = command_output_with_timeout(
            script.to_str().unwrap(),
            "--version",
            Duration::from_millis(100),
        );
        assert!(output.is_none());
    }
}
