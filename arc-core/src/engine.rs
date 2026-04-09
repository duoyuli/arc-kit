use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use log::info;

use crate::adapters::base::{AgentContext, Snapshot};
use crate::adapters::registry::all_resource_adapters;
use crate::agent::{SkillInstallStrategy, agent_spec, project_skill_path, resource_install_subdir};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::models::{ResourceInfo, ResourceKind};

#[derive(Debug, Clone)]
pub struct InstallEngine {
    cache: DetectCache,
}

impl InstallEngine {
    pub fn new(cache: DetectCache) -> Self {
        Self { cache }
    }

    /// Returns true if this agent was detected (has a home root) in the current cache.
    pub fn is_agent_detected(&self, agent_id: &str) -> bool {
        self.cache
            .get_agent(agent_id)
            .is_some_and(|info| info.root.is_some())
    }

    pub fn install(
        &self,
        resource: &ResourceInfo,
        source_path: &Path,
        targets: &[String],
    ) -> Result<Vec<String>> {
        self.install_named(&resource.name, &resource.kind, source_path, targets)
    }

    pub fn install_named(
        &self,
        name: &str,
        kind: &ResourceKind,
        source_path: &Path,
        targets: &[String],
    ) -> Result<Vec<String>> {
        let adapters = all_resource_adapters();
        let mut installed = Vec::new();
        for target in targets {
            let agent_info = self.cache.get_agent(target).ok_or_else(|| {
                ArcError::with_hint(
                    format!("Agent '{target}' not detected"),
                    format!("Install {target} first or choose a different agent"),
                )
            })?;
            let ctx =
                agent_context(target, agent_info.root.clone()).expect("detected agent has root");
            let snapshot = Snapshot {
                name: name.to_string(),
                kind: kind.clone(),
                path: source_path.to_path_buf(),
                metadata: BTreeMap::new(),
            };
            let mut matched = false;
            for adapter in &adapters {
                if adapter.supports(&snapshot, &ctx) {
                    let result = adapter.apply(&snapshot, &ctx);
                    if !result.ok {
                        return Err(ArcError::new(result.message));
                    }
                    info!("installed {} → {} ({})", name, target, kind);
                    installed.push(target.clone());
                    matched = true;
                    break;
                }
            }
            if !matched {
                return Err(ArcError::new(format!(
                    "No adapter found for '{}' on agent '{}'",
                    kind, target
                )));
            }
        }
        Ok(installed)
    }

    /// Install a skill into **project-local** paths (e.g. `<repo>/.claude/skills/<name>`) for each
    /// detected target agent. Does not write to `~/.claude` etc.; use [`Self::install_named`] for
    /// global (user-home) installs.
    pub fn install_named_project(
        &self,
        name: &str,
        kind: &ResourceKind,
        source_path: &Path,
        project_root: &Path,
        targets: &[String],
    ) -> Result<Vec<String>> {
        if !matches!(kind, ResourceKind::Skill) {
            return Err(ArcError::new(
                "project install only supports skills (ResourceKind::Skill)",
            ));
        }
        let mut installed = Vec::new();
        for target in targets {
            self.cache.get_agent(target).ok_or_else(|| {
                ArcError::with_hint(
                    format!("Agent '{target}' not detected"),
                    format!("Install {target} first or choose a different agent"),
                )
            })?;
            let spec = agent_spec(target.as_str()).ok_or_else(|| {
                ArcError::new(format!("unknown agent id '{target}' for project install"))
            })?;
            let dest = project_skill_path(project_root, target, name).ok_or_else(|| {
                ArcError::new(format!("no project skill path for agent '{target}'"))
            })?;
            let Some(parent) = dest.parent() else {
                return Err(ArcError::new("invalid project skill destination"));
            };
            if let Err(err) = fs::create_dir_all(parent) {
                return Err(ArcError::new(format!(
                    "failed to create project skills dir: {err}"
                )));
            }
            if (dest.exists() || dest.symlink_metadata().is_ok())
                && fs::remove_file(&dest).is_err()
                && fs::remove_dir_all(&dest).is_err()
            {
                return Err(ArcError::new(format!(
                    "failed to replace existing project skill at {}",
                    dest.display()
                )));
            }
            match spec.skill_install_strategy {
                SkillInstallStrategy::Symlink => {
                    #[cfg(unix)]
                    {
                        if let Err(err) = std::os::unix::fs::symlink(source_path, &dest) {
                            return Err(ArcError::new(format!("failed to create symlink: {err}")));
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        return Err(ArcError::new(
                            "symlink project install is unsupported on this platform",
                        ));
                    }
                }
                SkillInstallStrategy::Copy => {
                    if let Err(err) = copy_dir_recursive(source_path, &dest) {
                        return Err(ArcError::new(format!("failed to copy skill: {err}")));
                    }
                }
            }
            info!(
                "installed {} → project {} ({})",
                name,
                dest.display(),
                target
            );
            installed.push(target.clone());
        }
        Ok(installed)
    }

    pub fn uninstall(
        &self,
        name: &str,
        kind: &ResourceKind,
        targets: Option<&[String]>,
    ) -> Result<bool> {
        let adapters = all_resource_adapters();
        let agents = self.cache.detected_agents();
        let selected_targets: Vec<String> = targets
            .map(|items| items.to_vec())
            .unwrap_or_else(|| agents.keys().cloned().collect());
        let mut removed = false;
        for target in selected_targets {
            let Some(agent_info) = agents.get(&target) else {
                continue;
            };
            let Some(root) = &agent_info.root else {
                continue;
            };
            let ctx = agent_context(&target, Some(root.clone())).expect("checked root");
            let snapshot = Snapshot {
                name: name.to_string(),
                kind: kind.clone(),
                path: PathBuf::new(),
                metadata: BTreeMap::new(),
            };
            for adapter in &adapters {
                if adapter.supports(&snapshot, &ctx) {
                    let result = adapter.uninstall(&snapshot, &ctx);
                    if result.ok && !result.applied.is_empty() {
                        info!("uninstalled {} from {}", name, target);
                        removed = true;
                    }
                    break;
                }
            }
        }
        Ok(removed)
    }

    pub fn is_installed_for(&self, name: &str, kind: &ResourceKind, target: &str) -> bool {
        let Some(agent_info) = self.cache.get_agent(target) else {
            return false;
        };
        let Some(root) = &agent_info.root else {
            return false;
        };
        self.resource_path(root, kind, name, target).exists()
    }

    pub fn is_installed(&self, name: &str, kind: &ResourceKind) -> bool {
        self.cache
            .detected_agents()
            .iter()
            .any(|(target, agent_info)| {
                agent_info
                    .root
                    .as_ref()
                    .is_some_and(|root| self.resource_path(root, kind, name, target).exists())
            })
    }

    pub fn get_installed_targets(&self, name: &str, kind: &ResourceKind) -> Vec<String> {
        self.cache
            .detected_agents()
            .iter()
            .filter_map(|(target, agent_info)| {
                agent_info.root.as_ref().and_then(|root| {
                    self.resource_path(root, kind, name, target)
                        .exists()
                        .then_some(target.clone())
                })
            })
            .collect()
    }

    pub fn list_installed(&self, kind: Option<&ResourceKind>) -> Vec<InstalledResource> {
        let agents = self.cache.detected_agents();
        let mut seen: BTreeMap<String, InstalledResource> = BTreeMap::new();
        let kinds = kind
            .map(|kind| vec![kind.clone()])
            .unwrap_or_else(|| vec![ResourceKind::Skill]);
        for current_kind in kinds {
            for (target, agent_info) in agents {
                let Some(root) = &agent_info.root else {
                    continue;
                };
                let resource_dir = root.join(resource_install_subdir(&current_kind, target));
                let Ok(entries) = std::fs::read_dir(&resource_dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let key = format!("{}/{}", current_kind.as_str(), name);
                    let installed = seen.entry(key).or_insert_with(|| InstalledResource {
                        name: name.clone(),
                        kind: current_kind.clone(),
                        targets: Vec::new(),
                    });
                    installed.targets.push(target.clone());
                }
            }
        }
        seen.into_values().collect()
    }

    pub fn resource_path(
        &self,
        agent_root: &Path,
        kind: &ResourceKind,
        name: &str,
        agent_name: &str,
    ) -> PathBuf {
        agent_root
            .join(resource_install_subdir(kind, agent_name))
            .join(name)
    }
}

#[derive(Debug, Clone)]
pub struct InstalledResource {
    pub name: String,
    pub kind: ResourceKind,
    pub targets: Vec<String>,
}

fn agent_context(name: &str, root: Option<PathBuf>) -> Option<AgentContext> {
    Some(AgentContext {
        name: name.to_string(),
        detected: true,
        root: root?,
    })
}

fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}
