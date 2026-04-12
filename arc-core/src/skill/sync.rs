//! Global skill maintenance after catalog refresh: remove installs whose skill name is no longer in
//! the merged registry, then re-apply installs so remaining skills point at the latest resolved
//! source (handles market layout changes without deleting the skill).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use log::info;

use crate::agent::agent_spec;
use crate::detect::DetectCache;
use crate::engine::InstallEngine;
use crate::error::Result;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::skill::SkillRegistry;
use crate::skill::tracking::{
    fingerprint_path, global_skill_target_needs_sync, list_tracked_global_skill_installs,
    track_global_skill_install, untrack_global_skill_install,
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
    let cleanup = registry.cleanup_removed_global_skills()?;
    let sync = registry.sync_installed_global_skills(&engine)?;
    Ok(GlobalSkillMaintenanceReport { cleanup, sync })
}

impl SkillRegistry {
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
