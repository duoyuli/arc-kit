//! Global skill maintenance after catalog refresh: remove installs whose skill name is no longer in
//! the merged registry, then re-apply installs so remaining skills point at the latest resolved
//! source (handles market layout changes without deleting the skill).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use log::info;

use crate::agent::SkillInstallStrategy;
use crate::detect::DetectCache;
use crate::engine::InstallEngine;
use crate::error::Result;
use crate::paths::ArcPaths;
use crate::skill::SkillRegistry;
use crate::skill::install::{
    InstallOperation, InstallRecord, InstallScope, InstallStore, inspect_owned,
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
    pub fn cleanup_removed_global_skills(&self) -> Result<GlobalSkillCleanupReport> {
        let known: HashSet<String> = self
            .list_all()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        let mut store = InstallStore::open(self.arc_paths())?;
        let agents: Vec<_> = store
            .ledger
            .in_scope(InstallScope::Global, None)
            .map(|record| record.agent.clone())
            .chain(
                store
                    .journals
                    .iter()
                    .filter(|journal| journal.context.scope == InstallScope::Global)
                    .map(|journal| journal.context.agent.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        store.recover(InstallScope::Global, None, &agents)?;
        let records: Vec<_> = store
            .ledger
            .in_scope(InstallScope::Global, None)
            .filter(|record| !known.contains(&record.skill))
            .cloned()
            .collect();
        let mut removed = 0;
        for record in records {
            let present = inspect_owned(self.arc_paths(), &record)?;
            let mut operation = InstallOperation::remove(record.clone());
            operation.metadata_only = !present;
            store.execute(operation)?;
            if present {
                removed += 1;
            }
            info!(
                "removed tracked global skill '{}' from {}",
                record.skill,
                record.target_path.display()
            );
        }
        Ok(GlobalSkillCleanupReport { removed })
    }

    pub fn sync_installed_global_skills(
        &self,
        _engine: &InstallEngine,
    ) -> Result<InstalledSkillSyncReport> {
        let entries: BTreeMap<_, _> = self
            .list_all()
            .into_iter()
            .map(|entry| (entry.name.clone(), entry))
            .collect();
        let mut store = InstallStore::open(self.arc_paths())?;
        let agents: Vec<_> = store
            .ledger
            .in_scope(InstallScope::Global, None)
            .map(|record| record.agent.clone())
            .chain(
                store
                    .journals
                    .iter()
                    .filter(|journal| journal.context.scope == InstallScope::Global)
                    .map(|journal| journal.context.agent.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        store.recover(InstallScope::Global, None, &agents)?;
        let installs: Vec<_> = store
            .ledger
            .in_scope(InstallScope::Global, None)
            .cloned()
            .collect();
        let mut report = InstalledSkillSyncReport::default();
        for install in installs {
            let Some(skill) = entries.get(&install.skill) else {
                continue;
            };
            let attempt = (|| -> Result<bool> {
                let present = inspect_owned(self.arc_paths(), &install)?;
                let source = self.resolve_source_path(skill)?;
                let after = InstallRecord::new(
                    InstallScope::Global,
                    None,
                    &install.agent,
                    &install.skill,
                    &source,
                    &install.target_path,
                    install.strategy,
                )?;
                let needs_sync = !present
                    || install.source_path != after.source_path
                    || (install.strategy == SkillInstallStrategy::Copy
                        && install.source_fingerprint != after.source_fingerprint);
                if needs_sync {
                    store.execute(InstallOperation::put(after, Some(install.clone()), false))?;
                }
                Ok(needs_sync)
            })();
            match attempt {
                Ok(true) => report.refreshed += 1,
                Ok(false) => {}
                Err(err) => report.failures.push(InstalledSkillSyncFailure {
                    skill: install.skill,
                    agent: Some(install.agent),
                    message: err.message,
                }),
            }
        }
        for legacy in &store.ledger.unresolved_legacy {
            report.failures.push(InstalledSkillSyncFailure {
                skill: legacy
                    .get("skill")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                agent: legacy
                    .get("agent")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                message: "legacy installation ownership is unresolved; target preserved"
                    .to_string(),
            });
        }
        Ok(report)
    }
}
