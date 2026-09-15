use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::resource_install_subdir;
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::models::{ResourceInfo, ResourceKind};
use crate::paths::ArcPaths;
use crate::skill::install::{
    InstallItem, InstallReport, InstallStatus, install_global_skill, install_project_skill,
    uninstall_global_skill,
};

#[derive(Debug, Clone)]
pub struct InstallEngine {
    cache: DetectCache,
    paths: Option<ArcPaths>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallResult {
    pub attempted_agents: Vec<String>,
    pub removed_agents: Vec<String>,
    pub items: Vec<InstallItem>,
}

impl UninstallResult {
    pub fn removed_any(&self) -> bool {
        !self.removed_agents.is_empty()
    }
}

impl InstallEngine {
    pub fn new(cache: DetectCache) -> Self {
        Self { cache, paths: None }
    }

    /// Construct an engine whose writes commit installation metadata.
    pub fn with_paths(paths: ArcPaths, cache: DetectCache) -> Self {
        Self {
            cache,
            paths: Some(paths),
        }
    }

    fn write_paths(&self) -> Result<&ArcPaths> {
        self.paths
            .as_ref()
            .ok_or_else(|| ArcError::new("installation writes require InstallEngine::with_paths"))
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
        source: &Path,
        targets: &[String],
    ) -> Result<Vec<String>> {
        let report = self.install_named_report(name, kind, source, targets)?;
        completed_agents(report)
    }

    pub fn install_named_report(
        &self,
        name: &str,
        kind: &ResourceKind,
        source: &Path,
        targets: &[String],
    ) -> Result<InstallReport> {
        if *kind != ResourceKind::Skill {
            return Err(ArcError::new("only skill installations are supported"));
        }
        Ok(install_global_skill(
            self.write_paths()?,
            &self.cache,
            name,
            source,
            targets,
        ))
    }

    pub fn install_named_project(
        &self,
        name: &str,
        kind: &ResourceKind,
        source: &Path,
        root: &Path,
        targets: &[String],
    ) -> Result<Vec<String>> {
        if *kind != ResourceKind::Skill {
            return Err(ArcError::new("project install only supports skills"));
        }
        completed_agents(install_project_skill(
            self.write_paths()?,
            &self.cache,
            root,
            name,
            source,
            targets,
        ))
    }

    pub fn uninstall(
        &self,
        name: &str,
        kind: &ResourceKind,
        targets: Option<&[String]>,
    ) -> Result<UninstallResult> {
        if *kind != ResourceKind::Skill {
            return Err(ArcError::new("only skill uninstallations are supported"));
        }
        let report = uninstall_global_skill(self.write_paths()?, &self.cache, name, targets);
        if !report.ok() {
            return Err(report_error(&report));
        }
        let mut attempted: std::collections::BTreeSet<String> = targets
            .map(|items| {
                items
                    .iter()
                    .filter(|agent| self.cache.get_agent(agent).is_some())
                    .cloned()
                    .collect()
            })
            .unwrap_or_else(|| self.cache.detected_agents().keys().cloned().collect());
        attempted.extend(report.items.iter().map(|item| item.agent.clone()));
        let removed = report
            .items
            .iter()
            .filter(|item| item.status == InstallStatus::Removed)
            .map(|item| item.agent.clone())
            .collect::<std::collections::BTreeSet<_>>();
        Ok(UninstallResult {
            attempted_agents: attempted.into_iter().collect(),
            removed_agents: removed.into_iter().collect(),
            items: report.items,
        })
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
                    if name.starts_with(".arc-install-") {
                        continue;
                    }
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

fn completed_agents(report: InstallReport) -> Result<Vec<String>> {
    if !report.ok() {
        return Err(report_error(&report));
    }
    Ok(report
        .items
        .into_iter()
        .map(|item| item.agent)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn report_error(report: &InstallReport) -> ArcError {
    let messages: Vec<_> = report
        .errors
        .iter()
        .cloned()
        .chain(report.items.iter().filter_map(|item| item.message.clone()))
        .collect();
    ArcError::new(if messages.is_empty() {
        "installation operation failed".to_string()
    } else {
        messages.join("; ")
    })
}
