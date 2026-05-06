use std::path::{Path, PathBuf};

use crate::detect::{DetectCache, project_skills_satisfied_all, project_skills_satisfied_any};
use crate::engine::InstallEngine;
use crate::error::Result;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::provider::{load_providers_for_agent, read_active_provider, supported_provider_agents};
use crate::skill::SkillRegistry;

use super::discover::find_project_config;
use super::file::{ProjectConfig, load_project_config};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Global,
    Project,
}

#[derive(Debug, Clone)]
pub struct Sourced<T> {
    pub value: T,
    pub source: ConfigSource,
}

#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    /// Inferred from the directory name containing arc.toml, or cwd.
    pub project_name: String,
    /// Path to arc.toml; None means no project config was found.
    pub config_path: Option<PathBuf>,
    /// Directory containing `arc.toml` (project root). None when no project config.
    pub project_root: Option<PathBuf>,
    pub provider: Option<Sourced<String>>,
    pub required_skills: Vec<String>,
    /// Required skills present under the project tree for **at least one** install-target agent.
    pub installed_skills: Vec<String>,
    /// Required skills in the catalog but not present under **any** project-local agent path.
    pub missing_skills_absent: Vec<String>,
    /// Required skills in the catalog but not yet replicated to **every** project-local agent path
    /// (includes [`Self::missing_skills_absent`]).
    pub missing_installable: Vec<String>,
    /// Required skills not found in any source at all.
    pub missing_unavailable: Vec<String>,
}

impl EffectiveConfig {
    /// True when every required skill is replicated to all project-local agent paths (or global
    /// install when there is no `arc.toml`). Unavailable skills are ignored.
    pub fn is_up_to_date(&self) -> bool {
        self.missing_installable.is_empty()
    }

    /// When `arc.toml` declares `[provider] name`, returns `Ok(Some(name))` if any agent with that
    /// profile is not yet on that provider; `Ok(None)` if already active everywhere or the active
    /// value comes from global state only.
    ///
    /// Returns `Err` if the project names a provider that does not exist in any
    /// `~/.arc-cli/providers/<agent>.toml` file.
    pub fn provider_to_switch(&self, paths: &ArcPaths) -> crate::error::Result<Option<&str>> {
        let Some(sourced) = self.provider.as_ref() else {
            return Ok(None);
        };
        if sourced.source != ConfigSource::Project {
            return Ok(None);
        }
        let name = sourced.value.as_str();
        let providers_dir = paths.providers_dir();

        let mut profile_found = false;
        for agent in supported_provider_agents() {
            let providers = load_providers_for_agent(&providers_dir, agent);
            if !providers.iter().any(|p| p.name == name) {
                continue;
            }
            profile_found = true;
            let active = read_active_provider(&providers_dir, agent);
            if active.as_deref() != Some(name) {
                return Ok(Some(name));
            }
        }

        if !profile_found {
            return Err(crate::error::ArcError::with_hint(
                format!("Provider '{name}' in arc.toml was not found under ~/.arc-cli/providers."),
                "Check the name matches `arc provider list`, or add a profile.".to_string(),
            ));
        }

        Ok(None)
    }
}

pub fn resolve_effective_config(
    paths: &ArcPaths,
    cwd: &Path,
    cache: &DetectCache,
    registry: &SkillRegistry,
) -> Result<EffectiveConfig> {
    let config_path = find_project_config(cwd);

    let project_name = config_path
        .as_ref()
        .and_then(|p| p.parent())
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            cwd.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".to_string())
        });

    let project_cfg = config_path
        .as_deref()
        .map(load_project_config)
        .transpose()?
        .unwrap_or_default();

    let provider = resolve_provider(paths, &project_cfg, cache);

    let required_skills = project_cfg.skills.require.clone();

    let project_root = config_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    let (installed_skills, missing_skills_absent, missing_installable, missing_unavailable) =
        classify_skills(cache, registry, &required_skills, project_root.as_deref());

    Ok(EffectiveConfig {
        project_name,
        config_path,
        project_root,
        provider,
        required_skills,
        installed_skills,
        missing_skills_absent,
        missing_installable,
        missing_unavailable,
    })
}

fn resolve_provider(
    paths: &ArcPaths,
    project_cfg: &ProjectConfig,
    _cache: &DetectCache,
) -> Option<Sourced<String>> {
    if let Some(name) = &project_cfg.provider.name {
        return Some(Sourced {
            value: name.clone(),
            source: ConfigSource::Project,
        });
    }
    // Fall back: pick the active provider for the first supported agent.
    let providers_dir = paths.providers_dir();
    for agent in supported_provider_agents() {
        if let Some(name) = read_active_provider(&providers_dir, agent) {
            return Some(Sourced {
                value: name,
                source: ConfigSource::Global,
            });
        }
    }
    None
}

/// Split `required` into installed (any agent), absent (no project path), pending replication
/// (`!all`), and unavailable.
///
/// When `project_root` is set (`arc.toml` present), **installed** uses at-least-one-agent
/// presence; **missing_installable** uses not-on-all-agents (for `arc project apply`). Otherwise
/// falls back to global user-home installs.
fn classify_skills(
    cache: &DetectCache,
    registry: &SkillRegistry,
    required: &[String],
    project_root: Option<&Path>,
) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let engine = InstallEngine::new(cache.clone());
    let mut installed = Vec::new();
    let mut absent = Vec::new();
    let mut installable = Vec::new();
    let mut unavailable = Vec::new();

    for name in required {
        let Some(_) = registry.find(name) else {
            unavailable.push(name.clone());
            continue;
        };

        if let Some(root) = project_root {
            let any = project_skills_satisfied_any(cache, root, name);
            let all = project_skills_satisfied_all(cache, root, name);
            if any {
                installed.push(name.clone());
            } else {
                absent.push(name.clone());
            }
            if !all {
                installable.push(name.clone());
            }
        } else if engine.is_installed(name, &ResourceKind::Skill) {
            installed.push(name.clone());
        } else {
            installable.push(name.clone());
        }
    }

    (installed, absent, installable, unavailable)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::detect::{AgentInfo, DetectCache};
    use crate::paths::ArcPaths;
    use crate::skill::SkillRegistry;

    fn make_env(home: &Path) -> (ArcPaths, DetectCache, SkillRegistry) {
        let paths = ArcPaths::with_user_home(home);
        let cache = DetectCache::new(&paths);
        let registry = SkillRegistry::new(paths.clone(), cache.clone());
        (paths, cache, registry)
    }

    #[test]
    fn no_project_config_uses_global_only() {
        let home = tempdir().unwrap();
        let cwd = tempdir().unwrap();
        let (paths, cache, registry) = make_env(home.path());

        let cfg = resolve_effective_config(&paths, cwd.path(), &cache, &registry).unwrap();
        assert!(cfg.config_path.is_none());
        assert!(cfg.required_skills.is_empty());
    }

    #[test]
    fn project_provider_overrides_global() {
        let home = tempdir().unwrap();
        let proj = tempdir().unwrap();
        fs::write(
            proj.path().join("arc.toml"),
            "[provider]\nname = \"aicodemirror\"\n",
        )
        .unwrap();

        let (paths, cache, registry) = make_env(home.path());
        let cfg = resolve_effective_config(&paths, proj.path(), &cache, &registry).unwrap();
        let provider = cfg.provider.unwrap();
        assert_eq!(provider.value, "aicodemirror");
        assert_eq!(provider.source, ConfigSource::Project);
    }

    #[test]
    fn classifies_skills_into_three_buckets() {
        let home = tempdir().unwrap();
        let proj = tempdir().unwrap();
        fs::write(
            proj.path().join("arc.toml"),
            "[skills]\nrequire = [\"ghost-skill\"]\n",
        )
        .unwrap();

        let (paths, cache, registry) = make_env(home.path());
        let cfg = resolve_effective_config(&paths, proj.path(), &cache, &registry).unwrap();
        // ghost-skill is unknown, so it ends up in missing_unavailable.
        assert!(cfg.missing_unavailable.contains(&"ghost-skill".to_string()));
    }

    #[test]
    fn project_name_inferred_from_directory() {
        let home = tempdir().unwrap();
        let proj = tempdir().unwrap();
        fs::write(proj.path().join("arc.toml"), "").unwrap();

        let (paths, cache, registry) = make_env(home.path());
        let cfg = resolve_effective_config(&paths, proj.path(), &cache, &registry).unwrap();
        // tempdir names are auto-generated; just assert it's non-empty.
        assert!(!cfg.project_name.is_empty());
    }

    #[test]
    fn project_provider_unknown_errors_on_switch_check() {
        let home = tempdir().unwrap();
        let proj = tempdir().unwrap();
        fs::write(
            proj.path().join("arc.toml"),
            "[provider]\nname = \"no-such-profile-xyz\"\n",
        )
        .unwrap();

        let (paths, cache, registry) = make_env(home.path());
        let cfg = resolve_effective_config(&paths, proj.path(), &cache, &registry).unwrap();
        let err = cfg.provider_to_switch(&paths).unwrap_err();
        assert!(err.message.contains("no-such-profile-xyz"));
    }

    #[test]
    fn partial_project_install_lists_installed_but_still_pending_replication() {
        let home = tempdir().unwrap();
        let proj = tempdir().unwrap();

        fs::create_dir_all(home.path().join(".arc-cli/skills/partial-skill")).unwrap();
        fs::write(
            home.path().join(".arc-cli/skills/partial-skill/SKILL.md"),
            "# partial\n",
        )
        .unwrap();

        fs::write(
            proj.path().join("arc.toml"),
            "[skills]\nrequire = [\"partial-skill\"]\n",
        )
        .unwrap();

        let claude_dir = proj.path().join(".claude/skills/partial-skill");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("SKILL.md"), "# x\n").unwrap();

        let paths = ArcPaths::with_user_home(home.path());
        let mut agents = BTreeMap::new();
        agents.insert(
            "claude".to_string(),
            AgentInfo {
                name: "claude".to_string(),
                detected: true,
                root: Some(paths.user_home().join(".claude")),
                executable: Some("/fake/claude".to_string()),
                version: Some("1".to_string()),
            },
        );
        agents.insert(
            "codex".to_string(),
            AgentInfo {
                name: "codex".to_string(),
                detected: true,
                root: Some(paths.user_home().join(".codex")),
                executable: Some("/fake/codex".to_string()),
                version: Some("1".to_string()),
            },
        );
        let cache = DetectCache::from_map(agents);
        let registry = SkillRegistry::new(paths.clone(), cache.clone());

        let cfg = resolve_effective_config(&paths, proj.path(), &cache, &registry).unwrap();
        assert!(
            cfg.installed_skills.contains(&"partial-skill".to_string()),
            "expected skill counted installed when present under one agent path"
        );
        assert!(cfg.missing_skills_absent.is_empty());
        assert!(
            cfg.missing_installable
                .contains(&"partial-skill".to_string()),
            "expected replication pending until all project-capable agents have the skill"
        );
        assert!(!cfg.is_up_to_date());
    }
}
