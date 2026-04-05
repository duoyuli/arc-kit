//! Global skill maintenance after catalog refresh: remove installs whose skill name is no longer in
//! the merged registry, then re-apply installs so remaining skills point at the latest resolved
//! source (handles market layout changes without deleting the skill).

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use log::info;

use crate::detect::{CODING_AGENTS, DetectCache};
use crate::engine::InstallEngine;
use crate::error::Result;
use crate::models::ResourceKind;
use crate::paths::ArcPaths;
use crate::skill::SkillRegistry;

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

        for spec in CODING_AGENTS.iter() {
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
                if known.contains(&name) {
                    continue;
                }
                let path = entry.path();
                if !(path.is_symlink() || path.is_dir()) {
                    continue;
                }
                remove_skill_path(&path)?;
                removed += 1;
                info!(
                    "removed global skill '{}' (no longer in registry) from {}",
                    name, spec.id
                );
            }
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
        let mut report = InstalledSkillSyncReport::default();

        for skill in entries {
            if skill.installed_targets.is_empty() {
                continue;
            }
            let targets: Vec<String> = skill
                .installed_targets
                .iter()
                .filter(|id| engine.is_agent_detected(id))
                .cloned()
                .collect();
            if targets.is_empty() {
                continue;
            }
            let source_path = match self.resolve_source_path(&skill) {
                Ok(p) => p,
                Err(e) => {
                    report.failures.push(InstalledSkillSyncFailure {
                        skill: skill.name.clone(),
                        agent: None,
                        message: e.message,
                    });
                    continue;
                }
            };
            for target in targets {
                match engine.install_named(
                    &skill.name,
                    &ResourceKind::Skill,
                    &source_path,
                    std::slice::from_ref(&target),
                ) {
                    Ok(_) => {
                        report.refreshed += 1;
                        info!("synced global skill '{}' → {}", skill.name, target);
                    }
                    Err(e) => {
                        report.failures.push(InstalledSkillSyncFailure {
                            skill: skill.name.clone(),
                            agent: Some(target),
                            message: e.message,
                        });
                    }
                }
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
