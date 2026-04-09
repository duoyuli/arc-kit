//! Global skill maintenance after catalog refresh: remove installs whose skill name is no longer in
//! the merged registry, then re-apply installs so remaining skills point at the latest resolved
//! source (handles market layout changes without deleting the skill).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use log::info;

use crate::agent::{SkillInstallStrategy, agent_spec, agent_specs};
use crate::detect::DetectCache;
use crate::engine::InstallEngine;
use crate::error::Result;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::skill::SkillRegistry;
use crate::skill::tracking::{
    fingerprint_path, global_skill_target_needs_sync, list_tracked_global_skill_installs,
    track_global_skill_install, tracking_file_path, untrack_global_skill_install,
};

/// Report from removing global installs whose name is absent from the merged registry.
#[derive(Debug, Clone, Default)]
pub struct GlobalSkillCleanupReport {
    pub removed: usize,
}

/// One failed per-target sync step.
#[derive(Debug, Clone)]
pub struct InstalledSkillSyncFailure {
    pub skill: String,
    pub agent: Option<String>,
    pub message: String,
}

/// Report from re-installing every globally installed skill from current [`SkillRegistry::resolve_source_path`].
#[derive(Debug, Clone, Default)]
pub struct InstalledSkillSyncReport {
    pub refreshed: usize,
    pub failures: Vec<InstalledSkillSyncFailure>,
}

/// Cleanup then sync; run after market catalog rebuild.
#[derive(Debug, Clone, Default)]
pub struct GlobalSkillMaintenanceReport {
    pub cleanup: GlobalSkillCleanupReport,
    pub sync: InstalledSkillSyncReport,
}

/// Run global skill cleanup and sync. Call after the skill catalog reflects the latest market state.
pub fn run_global_skill_maintenance(
    paths: &ArcPaths,
    cache: &DetectCache,
) -> Result<GlobalSkillMaintenanceReport> {
    paths
        .ensure_arc_home()
        .map_err(|e: std::io::Error| crate::error::ArcError::new(e.to_string()))?;
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let engine = InstallEngine::new(cache.clone());
    registry.bootstrap_legacy_global_skill_tracking()?;
    let cleanup = registry.cleanup_removed_global_skills()?;
    let sync = registry.sync_installed_global_skills(&engine)?;
    Ok(GlobalSkillMaintenanceReport { cleanup, sync })
}

impl SkillRegistry {
    fn bootstrap_legacy_global_skill_tracking(&self) -> Result<()> {
        for spec in agent_specs() {
            if !spec.supports_skills {
                continue;
            }
            let Some(agent_info) = self.detect_cache().get_agent(spec.id) else {
                continue;
            };
            let Some(root) = &agent_info.root else {
                continue;
            };
            let skills_dir = root.join(spec.skills_subdir);
            let Ok(rd) = fs::read_dir(&skills_dir) else {
                continue;
            };
            for entry in rd.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let target_path = entry.path();
                if !target_path.is_symlink() && !target_path.is_dir() {
                    continue;
                }
                let metadata_path = tracking_file_path(&skills_dir, &name);
                if metadata_path.exists() {
                    continue;
                }

                match spec.skill_install_strategy {
                    SkillInstallStrategy::Symlink => {
                        let Ok(link_target) = fs::read_link(&target_path) else {
                            continue;
                        };
                        let source_path = absolutize_link_target(&target_path, &link_target);
                        if !is_arc_managed_source_path(self.arc_paths(), &source_path) {
                            continue;
                        }
                        track_global_skill_install(&skills_dir, spec.id, &name, &source_path)?;
                    }
                    SkillInstallStrategy::Copy => {
                        let Some(skill) = self.find(&name) else {
                            continue;
                        };
                        let source_path = match self.resolve_source_path(&skill) {
                            Ok(path) => path,
                            Err(_) => continue,
                        };
                        let desired_fingerprint = fingerprint_path(&source_path)?;
                        if fingerprint_path(&target_path)? != desired_fingerprint {
                            continue;
                        }
                        track_global_skill_install(&skills_dir, spec.id, &name, &source_path)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove `~/<agent>/…/skills/<name>` entries when `<name>` is not in the merged registry
    /// (local > built-in > market). Skills that still exist are left for [`Self::sync_installed_global_skills`].
    pub fn cleanup_removed_global_skills(&self) -> Result<GlobalSkillCleanupReport> {
        self.arc_paths()
            .ensure_arc_home()
            .map_err(|e: std::io::Error| crate::error::ArcError::new(e.to_string()))?;
        let known: HashSet<String> = self.list_all().into_iter().map(|e| e.name).collect();
        let mut removed = 0usize;

        for install in list_tracked_global_skill_installs(self.detect_cache()) {
            if known.contains(&install.skill) {
                continue;
            }
            if target_exists(&install.target_path) {
                remove_skill_path(&install.target_path)?;
                removed += 1;
                info!(
                    "removed tracked global skill '{}' (no longer in registry) from {}",
                    install.skill, install.agent
                );
            }
            untrack_global_skill_install(&install.skills_dir, &install.skill)?;
        }

        Ok(GlobalSkillCleanupReport { removed })
    }

    /// Re-apply each globally installed skill so symlinks / copies match [`Self::resolve_source_path`].
    pub fn sync_installed_global_skills(
        &self,
        engine: &InstallEngine,
    ) -> Result<InstalledSkillSyncReport> {
        self.arc_paths()
            .ensure_arc_home()
            .map_err(|e: std::io::Error| crate::error::ArcError::new(e.to_string()))?;
        let mut entries = self.list_all();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let entries_by_name: BTreeMap<String, crate::models::SkillEntry> = entries
            .into_iter()
            .map(|entry| (entry.name.clone(), entry))
            .collect();
        let mut installs = list_tracked_global_skill_installs(self.detect_cache());
        installs.sort_by(|a, b| (&a.skill, &a.agent).cmp(&(&b.skill, &b.agent)));
        let mut report = InstalledSkillSyncReport::default();

        for install in installs {
            let Some(skill) = entries_by_name.get(&install.skill) else {
                continue;
            };
            if !engine.is_agent_detected(&install.agent) {
                continue;
            }
            let source_path = match self.resolve_source_path(skill) {
                Ok(p) => p,
                Err(e) => {
                    report.failures.push(InstalledSkillSyncFailure {
                        skill: install.skill.clone(),
                        agent: Some(install.agent.clone()),
                        message: e.message,
                    });
                    continue;
                }
            };
            let desired_fingerprint = match fingerprint_path(&source_path) {
                Ok(fingerprint) => fingerprint,
                Err(e) => {
                    report.failures.push(InstalledSkillSyncFailure {
                        skill: install.skill.clone(),
                        agent: Some(install.agent.clone()),
                        message: e.message,
                    });
                    continue;
                }
            };
            let Some(spec) = agent_spec(&install.agent) else {
                continue;
            };

            let needs_sync = match global_skill_target_needs_sync(
                &install.target_path,
                spec.skill_install_strategy,
                &source_path,
                &desired_fingerprint,
            ) {
                Ok(needs_sync) => needs_sync,
                Err(e) => {
                    report.failures.push(InstalledSkillSyncFailure {
                        skill: install.skill.clone(),
                        agent: Some(install.agent.clone()),
                        message: e.message,
                    });
                    continue;
                }
            };

            if needs_sync {
                match engine.install_named(
                    &skill.name,
                    &ResourceKind::Skill,
                    &source_path,
                    std::slice::from_ref(&install.agent),
                ) {
                    Ok(_) => {
                        report.refreshed += 1;
                        info!(
                            "synced tracked global skill '{}' → {}",
                            skill.name, install.agent
                        );
                    }
                    Err(e) => {
                        report.failures.push(InstalledSkillSyncFailure {
                            skill: skill.name.clone(),
                            agent: Some(install.agent.clone()),
                            message: e.message,
                        });
                        continue;
                    }
                }
            }

            if needs_sync
                || install.source_path != source_path
                || install.source_fingerprint != desired_fingerprint
            {
                track_global_skill_install(
                    &install.skills_dir,
                    &install.agent,
                    &install.skill,
                    &source_path,
                )?;
            }
        }

        Ok(report)
    }
}

fn remove_skill_path(path: &Path) -> Result<()> {
    if path.is_symlink() {
        fs::remove_file(path).map_err(|e| {
            crate::error::ArcError::new(format!(
                "failed to remove stale skill symlink {}: {e}",
                path.display()
            ))
        })?;
    } else if path.is_file() {
        fs::remove_file(path).map_err(|e| {
            crate::error::ArcError::new(format!(
                "failed to remove stale skill file {}: {e}",
                path.display()
            ))
        })?;
    } else {
        fs::remove_dir_all(path).map_err(|e| {
            crate::error::ArcError::new(format!(
                "failed to remove stale skill directory {}: {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn target_exists(path: &Path) -> bool {
    path.exists() || path.symlink_metadata().is_ok()
}

fn absolutize_link_target(link_path: &Path, target: &Path) -> std::path::PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }
    link_path
        .parent()
        .map(|parent| parent.join(target))
        .unwrap_or_else(|| target.to_path_buf())
}

fn is_arc_managed_source_path(paths: &ArcPaths, source_path: &Path) -> bool {
    source_path.starts_with(paths.local_skills_dir())
        || source_path.starts_with(paths.builtin_cache_dir())
        || source_path.starts_with(paths.markets_repo_root())
}
