use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use crate::agent::{AgentSpec, agent_spec, agent_specs, default_install_targets};
use crate::models::ResourceKind;
use crate::paths::ArcPaths;

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
        let ordered = crate::agent::ordered_agent_ids_for_resource_kind(kind);
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

    /// Like [`Self::agents_for_install`], but only agents that support project-local skill dirs.
    pub fn agents_for_project_skill_install(&self, kind: &ResourceKind) -> Vec<String> {
        self.agents_for_install(kind)
            .into_iter()
            .filter(|id| {
                agent_spec(id)
                    .map(|spec| spec.supports_project_skills)
                    .unwrap_or(false)
            })
            .collect()
    }
}

fn parallel_detect_all(paths: &ArcPaths) -> BTreeMap<String, AgentInfo> {
    std::thread::scope(|s| {
        let handles: Vec<_> = agent_specs()
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

pub fn detect_agent(paths: &ArcPaths, name: &str) -> Result<AgentInfo, String> {
    let spec = agent_spec(name).ok_or_else(|| format!("unknown agent: {name}"))?;
    Ok(detect_from_spec(paths, spec))
}

pub fn detect_all_agents(paths: &ArcPaths) -> BTreeMap<String, AgentInfo> {
    agent_specs()
        .iter()
        .map(|spec| (spec.id.to_string(), detect_from_spec(paths, spec)))
        .filter(|(_, info)| info.detected)
        .collect()
}

pub fn get_detected_agents(paths: &ArcPaths) -> Vec<String> {
    detect_all_agents(paths).into_keys().collect()
}

pub fn detect_agents_for_install(paths: &ArcPaths, kind: &ResourceKind) -> Vec<String> {
    let ordered = crate::agent::ordered_agent_ids_for_resource_kind(kind);
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

/// True if `skill_name` exists under the project tree for **every** detected agent that supports
/// project-local skills. Used by `arc project apply` to decide whether replication is still needed.
pub fn project_skills_satisfied_all(
    cache: &DetectCache,
    project_root: &std::path::Path,
    skill_name: &str,
) -> bool {
    let targets = cache.agents_for_project_skill_install(&ResourceKind::Skill);
    if targets.is_empty() {
        return false;
    }
    targets.iter().all(|agent_id| {
        crate::agent::project_skill_path(project_root, agent_id, skill_name)
            .map(|path| path.exists())
            .unwrap_or(false)
    })
}

/// True if `skill_name` exists under the project tree for **at least one** such agent.
/// Used by `arc status` to report whether anything is materialized in the repo.
pub fn project_skills_satisfied_any(
    cache: &DetectCache,
    project_root: &std::path::Path,
    skill_name: &str,
) -> bool {
    let targets = cache.agents_for_project_skill_install(&ResourceKind::Skill);
    if targets.is_empty() {
        return false;
    }
    targets.iter().any(|agent_id| {
        crate::agent::project_skill_path(project_root, agent_id, skill_name)
            .map(|path| path.exists())
            .unwrap_or(false)
    })
}

fn detect_from_spec(paths: &ArcPaths, spec: &AgentSpec) -> AgentInfo {
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

fn detect_executable(spec: &AgentSpec) -> (Option<String>, Option<String>) {
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
        .filter(|value| !value.is_empty())
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
        .find(|token| token.starts_with(|ch: char| ch.is_ascii_digit()) && token.contains('.'))
        .map(|token| token.to_string())
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
