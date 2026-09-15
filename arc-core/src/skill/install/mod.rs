//! 按范围记录安装归属，并提供可恢复的逐目标文件系统操作。

mod global;
mod paths;
mod state;
mod transaction;

pub use global::{install_global_skill, install_project_skill, uninstall_global_skill};
pub(crate) use paths::{
    candidate_destinations, inspect_owned, validate_destination, validate_name,
};
pub use paths::{normalize_destination, normalize_root};
pub use state::{InstallLedger, InstallRecord, InstallScope, inspect_install_ledger};
pub(crate) use state::{InstallStore, Journal, with_snapshot};
pub(crate) use transaction::{InstallOperation, NodeState};

use crate::agent::SkillInstallStrategy;
use crate::error::{ArcError, Result};
use crate::skill::tracking::fingerprint_path;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallAction {
    Inspect,
    Keep,
    Install,
    Refresh,
    Adopt,
    Remove,
    Forget,
    Recover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    Planned,
    Kept,
    Installed,
    Refreshed,
    Adopted,
    Removed,
    Absent,
    Recovered,
    Unmanaged,
    Conflict,
    Unavailable,
    Unresolved,
    Failed,
    NotExecuted,
    CleanupPending,
}

impl InstallStatus {
    pub fn is_issue(self) -> bool {
        matches!(
            self,
            Self::Unmanaged
                | Self::Conflict
                | Self::Unavailable
                | Self::Unresolved
                | Self::Failed
                | Self::NotExecuted
                | Self::CleanupPending
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct InstallItem {
    pub scope: InstallScope,
    pub project_root: Option<std::path::PathBuf>,
    pub agent: String,
    pub skill: String,
    pub source_path: Option<std::path::PathBuf>,
    pub target_path: Option<std::path::PathBuf>,
    pub action: InstallAction,
    pub status: InstallStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl InstallItem {
    pub(crate) fn from_record(
        record: &InstallRecord,
        action: InstallAction,
        status: InstallStatus,
    ) -> Self {
        Self {
            scope: record.scope,
            project_root: record.project_root.clone(),
            agent: record.agent.clone(),
            skill: record.skill.clone(),
            source_path: Some(record.source_path.clone()),
            target_path: Some(record.target_path.clone()),
            action,
            status,
            message: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallReport {
    pub scope: InstallScope,
    pub project_root: Option<std::path::PathBuf>,
    pub dry_run: bool,
    pub items: Vec<InstallItem>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct InstallTrackingStatus {
    pub global_installs: usize,
    pub project_installs: usize,
    pub unresolved_legacy: Vec<serde_json::Value>,
    pub pending_operations: Vec<InstallItem>,
    pub error: Option<String>,
}

pub fn inspect_install_tracking(paths: &crate::paths::ArcPaths) -> InstallTrackingStatus {
    match with_snapshot(paths, |ledger, journals| {
        Ok(InstallTrackingStatus {
            global_installs: ledger
                .installs
                .iter()
                .filter(|record| record.scope == InstallScope::Global)
                .count(),
            project_installs: ledger
                .installs
                .iter()
                .filter(|record| record.scope == InstallScope::Project)
                .count(),
            unresolved_legacy: ledger.unresolved_legacy.clone(),
            pending_operations: journals
                .iter()
                .map(|journal| {
                    InstallItem::from_record(
                        &journal.context,
                        InstallAction::Recover,
                        InstallStatus::Unresolved,
                    )
                })
                .collect(),
            error: ledger.recovery_required.then(|| {
                "install ledger history was lost; pending operations require review".to_string()
            }),
        })
    }) {
        Ok(status) => status,
        Err(err) => InstallTrackingStatus {
            error: Some(err.message),
            ..Default::default()
        },
    }
}

impl InstallReport {
    pub fn new(
        scope: InstallScope,
        project_root: Option<std::path::PathBuf>,
        dry_run: bool,
    ) -> Self {
        Self {
            scope,
            project_root,
            dry_run,
            items: Vec::new(),
            errors: Vec::new(),
        }
    }
    pub fn ok(&self) -> bool {
        self.errors.is_empty() && !self.items.iter().any(|item| item.status.is_issue())
    }
    pub fn has_changes(&self) -> bool {
        self.items
            .iter()
            .any(|item| !matches!(item.action, InstallAction::Inspect | InstallAction::Keep))
    }
}

impl InstallRecord {
    pub fn new(
        scope: InstallScope,
        project_root: Option<&Path>,
        agent: &str,
        skill: &str,
        source_path: &Path,
        target_path: &Path,
        strategy: SkillInstallStrategy,
    ) -> Result<Self> {
        validate_name(skill)?;
        let source_metadata = std::fs::metadata(source_path).map_err(|err| {
            ArcError::new(format!(
                "source unavailable {}: {err}",
                source_path.display()
            ))
        })?;
        if !source_metadata.is_dir() {
            return Err(ArcError::new(format!(
                "skill source is not a directory: {}",
                source_path.display()
            )));
        }
        Ok(Self {
            scope,
            project_root: project_root.map(normalize_root).transpose()?,
            agent: agent.to_string(),
            skill: skill.to_string(),
            source_path: paths::absolute_lexical(source_path)?,
            target_path: normalize_destination(target_path)?,
            strategy,
            source_fingerprint: source_fingerprint(source_path, strategy)?,
            target_fingerprint: None,
        })
    }

    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        self.scope == other.scope
            && self.project_root == other.project_root
            && self.agent == other.agent
            && self.skill == other.skill
            && self.target_path == other.target_path
    }

    pub(crate) fn copy_fingerprint(&self) -> Result<&str> {
        self.target_fingerprint
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ArcError::new(format!(
                    "missing installed copy fingerprint for {}",
                    self.target_path.display()
                ))
            })
    }
}

pub(crate) fn source_fingerprint(path: &Path, strategy: SkillInstallStrategy) -> Result<String> {
    if strategy == SkillInstallStrategy::Copy {
        let source = std::fs::canonicalize(path).map_err(|err| {
            ArcError::new(format!("source unavailable {}: {err}", path.display()))
        })?;
        fingerprint_path(&source)
    } else {
        fingerprint_path(path)
    }
}
