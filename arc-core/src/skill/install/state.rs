use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::error::{ArcError, Result};
use crate::io::{atomic_write_bytes, now_unix_secs};
use crate::paths::ArcPaths;
use crate::skill::tracking::{fingerprint_path, legacy_fingerprint_path};

use super::paths::{
    absolute_lexical, candidate_destinations, io_error, link_destination, sync_directory,
};
use super::transaction::NodeState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallScope {
    Global,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    pub scope: InstallScope,
    pub project_root: Option<PathBuf>,
    pub agent: String,
    pub skill: String,
    pub target_path: PathBuf,
    pub source_path: PathBuf,
    pub strategy: SkillInstallStrategy,
    pub source_fingerprint: String,
    pub target_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallLedger {
    pub schema_version: u32,
    pub installs: Vec<InstallRecord>,
    #[serde(default)]
    pub unresolved_legacy: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) committed_operations: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) recovery_required: bool,
}

impl Default for InstallLedger {
    fn default() -> Self {
        Self {
            schema_version: 2,
            installs: Vec::new(),
            unresolved_legacy: Vec::new(),
            committed_operations: BTreeSet::new(),
            recovery_required: false,
        }
    }
}

impl InstallLedger {
    pub fn record(&self, target: &Path) -> Option<&InstallRecord> {
        self.installs
            .iter()
            .find(|record| record.target_path == target)
    }

    pub fn in_scope<'a>(
        &'a self,
        scope: InstallScope,
        root: Option<&'a Path>,
    ) -> impl Iterator<Item = &'a InstallRecord> {
        self.installs
            .iter()
            .filter(move |record| record.scope == scope && record.project_root.as_deref() == root)
    }

    fn validate(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for record in &self.installs {
            if !record.target_path.is_absolute()
                || !record.source_path.is_absolute()
                || (record.scope == InstallScope::Project) != record.project_root.is_some()
                || !seen.insert(&record.target_path)
            {
                return Err(ArcError::new("invalid or duplicate installation record"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JournalPhase {
    Prepared,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Journal {
    pub schema_version: u32,
    pub id: String,
    pub before: Option<InstallRecord>,
    pub after: Option<InstallRecord>,
    pub context: InstallRecord,
    pub workspace: PathBuf,
    pub workspace_identity: Option<(u64, u64)>,
    pub before_state: Option<NodeState>,
    pub after_state: Option<NodeState>,
    pub phase: JournalPhase,
    pub metadata_only: bool,
}

impl Journal {
    pub fn selected(&self, scope: InstallScope, root: Option<&Path>, agents: &[String]) -> bool {
        self.context.scope == scope
            && self.context.project_root.as_deref() == root
            && agents.contains(&self.context.agent)
    }
}

pub(crate) fn lock_path(paths: &ArcPaths) -> PathBuf {
    paths.state_dir().join("skills/installs.lock")
}

pub(crate) fn operations_dir(paths: &ArcPaths) -> PathBuf {
    paths.state_dir().join("skills/operations")
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(body) => Ok(Some(body)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io_error("cannot read install metadata", path, err)),
    }
}

fn migrate_legacy(paths: &ArcPaths, records: Vec<serde_json::Value>) -> InstallLedger {
    let mut ledger = InstallLedger::default();
    for value in records {
        let resolved = (|| -> Option<InstallRecord> {
            let agent = value.get("agent")?.as_str()?;
            let skill = value.get("skill")?.as_str()?;
            let source_path = Path::new(value.get("source_path")?.as_str()?);
            if !source_path.is_absolute() {
                return None;
            }
            let source = absolute_lexical(source_path).ok()?;
            let fingerprint = value
                .get("source_fingerprint")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let strategy = agent_spec(agent)?.skill_install_strategy;
            let candidates =
                candidate_destinations(paths, InstallScope::Global, None, agent, skill).ok()?;
            let matched: Vec<_> = candidates
                .into_iter()
                .filter_map(|target| {
                    let metadata = fs::symlink_metadata(&target).ok()?;
                    let owned = match strategy {
                        SkillInstallStrategy::Symlink => {
                            metadata.file_type().is_symlink()
                                && link_destination(&target).ok()? == source
                        }
                        SkillInstallStrategy::Copy => {
                            metadata.is_dir()
                                && !metadata.file_type().is_symlink()
                                && !fingerprint.is_empty()
                                && verified_legacy_copy_fingerprint(&target, &source, fingerprint)
                                    .is_some()
                        }
                    };
                    owned.then_some(target)
                })
                .collect();
            if matched.len() != 1 {
                return None;
            }
            let target_path = matched.into_iter().next()?;
            if ledger.record(&target_path).is_some() {
                return None;
            }
            let copy_fingerprint = if strategy == SkillInstallStrategy::Copy {
                Some(verified_legacy_copy_fingerprint(
                    &target_path,
                    &source,
                    fingerprint,
                )?)
            } else {
                None
            };
            Some(InstallRecord {
                scope: InstallScope::Global,
                project_root: None,
                agent: agent.to_string(),
                skill: skill.to_string(),
                target_path,
                source_path: source,
                strategy,
                source_fingerprint: copy_fingerprint
                    .clone()
                    .unwrap_or_else(|| fingerprint.to_string()),
                target_fingerprint: copy_fingerprint,
            })
        })();
        if let Some(record) = resolved {
            ledger.installs.push(record);
        } else {
            ledger.unresolved_legacy.push(value);
        }
    }
    ledger
}

fn verified_legacy_copy_fingerprint(
    target: &Path,
    source: &Path,
    expected: &str,
) -> Option<String> {
    let actual = fingerprint_path(target).ok()?;
    if expected.starts_with("sha256:") {
        return (actual == expected).then_some(actual);
    }
    // 旧指纹存在拼接歧义；必须同时核对当前来源，无法验证时保留原始记录。
    if legacy_fingerprint_path(target).ok()? != expected {
        return None;
    }
    let source = fs::canonicalize(source).ok()?;
    (fingerprint_path(&source).ok()? == actual).then_some(actual)
}

fn parse_ledger(paths: &ArcPaths, body: Option<&[u8]>) -> Result<(InstallLedger, bool)> {
    let Some(body) = body else {
        return Ok((InstallLedger::default(), false));
    };
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| {
        ArcError::new(
            "install metadata is corrupt; existing targets will not be adopted automatically",
        )
    })?;
    if let Some(records) = value.as_array() {
        return Ok((migrate_legacy(paths, records.clone()), true));
    }
    if value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
    {
        return Err(ArcError::new(
            "unsupported install ledger schema; refusing to write",
        ));
    }
    let ledger: InstallLedger = serde_json::from_value(value)
        .map_err(|_| ArcError::new("install metadata contains invalid records"))?;
    ledger.validate()?;
    Ok((ledger, false))
}

fn journal_bytes(paths: &ArcPaths) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let directory = operations_dir(paths);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(io_error("cannot inspect pending installs", &directory, err)),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|err| io_error("cannot inspect pending install", &directory, err))?;
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            result.push((
                entry.path(),
                fs::read(entry.path())
                    .map_err(|err| io_error("cannot read pending install", &entry.path(), err))?,
            ));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

fn parse_journals(bytes: &[(PathBuf, Vec<u8>)]) -> Result<Vec<Journal>> {
    bytes
        .iter()
        .map(|(path, body)| {
            let journal: Journal = serde_json::from_slice(body).map_err(|_| {
                io_error(
                    "invalid pending installation",
                    path,
                    "manual review required",
                )
            })?;
            if journal.schema_version != 1
                || path.file_stem().and_then(|name| name.to_str()) != Some(journal.id.as_str())
            {
                return Err(io_error(
                    "invalid pending installation identity",
                    path,
                    "refusing recovery",
                ));
            }
            Ok(journal)
        })
        .collect()
}

/// 检查文件系统期间持有共享锁，不初始化状态目录。
pub(crate) fn with_snapshot<T>(
    paths: &ArcPaths,
    inspect: impl FnOnce(&InstallLedger, &[Journal]) -> Result<T>,
) -> Result<T> {
    let lock = match File::open(lock_path(paths)) {
        Ok(file) => {
            file.try_lock_shared().map_err(|_| {
                ArcError::new("install state is busy; retry the read-only snapshot")
            })?;
            Some(file)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(io_error("cannot open install lock", &lock_path(paths), err)),
    };
    let before = read_optional(&paths.skill_tracking_file())?;
    let pending_bytes = journal_bytes(paths)?;
    let (mut ledger, _) = parse_ledger(paths, before.as_deref())?;
    let journals = parse_journals(&pending_bytes)?;
    // 已有操作日志却没有台账时，无法分辨操作是否已提交；只读检查也必须报告。
    ledger.recovery_required |= before.is_none() && !journals.is_empty();
    let result = inspect(&ledger, &journals)?;
    if lock.is_none()
        && (lock_path(paths).exists()
            || before != read_optional(&paths.skill_tracking_file())?
            || pending_bytes != journal_bytes(paths)?)
    {
        return Err(ArcError::new(
            "install state changed during the read-only snapshot; retry",
        ));
    }
    Ok(result)
}

pub fn inspect_install_ledger(paths: &ArcPaths) -> Result<InstallLedger> {
    with_snapshot(paths, |ledger, _| Ok(ledger.clone()))
}

pub(crate) struct InstallStore {
    pub paths: ArcPaths,
    pub ledger: InstallLedger,
    pub journals: Vec<Journal>,
    _lock: File,
    #[cfg(test)]
    pub fault: Option<super::transaction::FailurePoint>,
    #[cfg(test)]
    pub fail_cleanup: bool,
}

impl InstallStore {
    pub fn open(paths: &ArcPaths) -> Result<Self> {
        if let Some(body) = read_optional(&paths.skill_tracking_file())?
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body)
            && value.is_object()
            && value.get("schema_version").is_some()
            && value
                .get("schema_version")
                .and_then(serde_json::Value::as_u64)
                != Some(2)
        {
            return Err(ArcError::new(
                "unsupported install ledger schema; refusing to write",
            ));
        }
        let lock_path = lock_path(paths);
        fs::create_dir_all(lock_path.parent().unwrap())
            .map_err(|err| io_error("cannot create install state directory", &lock_path, err))?;
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|err| io_error("cannot open install lock", &lock_path, err))?;
        lock.lock()
            .map_err(|err| io_error("cannot lock install metadata", &lock_path, err))?;
        let body = read_optional(&paths.skill_tracking_file())?;
        let mut quarantined = false;
        let (mut ledger, legacy) = match parse_ledger(paths, body.as_deref()) {
            Ok(result) => result,
            Err(err) if err.message.contains("unsupported install ledger schema") => {
                return Err(err);
            }
            Err(_) => {
                quarantine(paths)?;
                quarantined = true;
                (InstallLedger::default(), false)
            }
        };
        let journals = parse_journals(&journal_bytes(paths)?)?;
        if (quarantined || body.is_none()) && !journals.is_empty() {
            ledger.recovery_required = true;
        }
        let store = Self {
            paths: paths.clone(),
            ledger,
            journals,
            _lock: lock,
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            fail_cleanup: false,
        };
        if store.ledger.recovery_required {
            store.save()?;
        }
        if legacy {
            let backup = unique_backup(paths, "v1");
            atomic_write_bytes(&backup, body.as_deref().unwrap())
                .map_err(|err| io_error("cannot back up legacy installs", &backup, err))?;
            store.save()?;
        }
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        self.ledger.validate()?;
        let mut ledger = self.ledger.clone();
        ledger
            .installs
            .sort_by(|a, b| a.target_path.cmp(&b.target_path));
        let mut body =
            serde_json::to_vec_pretty(&ledger).map_err(|err| ArcError::new(err.to_string()))?;
        body.push(b'\n');
        atomic_write_bytes(&self.paths.skill_tracking_file(), &body).map_err(|err| {
            io_error(
                "cannot commit install metadata",
                &self.paths.skill_tracking_file(),
                err,
            )
        })?;
        sync_directory(self.paths.skill_tracking_file().parent().unwrap())
    }

    pub fn journal_path(&self, id: &str) -> PathBuf {
        operations_dir(&self.paths).join(format!("{id}.json"))
    }

    pub fn save_journal(&self, journal: &Journal) -> Result<()> {
        let body =
            serde_json::to_vec_pretty(journal).map_err(|err| ArcError::new(err.to_string()))?;
        let path = self.journal_path(&journal.id);
        atomic_write_bytes(&path, &body)
            .map_err(|err| io_error("cannot write pending installation", &path, err))?;
        sync_directory(path.parent().unwrap())
    }

    pub fn reload(&mut self) -> Result<()> {
        let body = read_optional(&self.paths.skill_tracking_file())?;
        self.ledger = parse_ledger(&self.paths, body.as_deref())?.0;
        self.journals = parse_journals(&journal_bytes(&self.paths)?)?;
        self.ledger.recovery_required |= body.is_none() && !self.journals.is_empty();
        Ok(())
    }

    pub fn committed_pending(&self, target: &Path) -> bool {
        self.journals.iter().any(|journal| {
            journal.context.target_path == target
                && (journal.phase == JournalPhase::Committed
                    || self.ledger.committed_operations.contains(&journal.id))
        })
    }
}

fn unique_backup(paths: &ArcPaths, kind: &str) -> PathBuf {
    let parent = paths.skill_tracking_file().parent().unwrap().to_path_buf();
    let mut suffix = 0usize;
    loop {
        let path = parent.join(format!("installs.{kind}.{}.{suffix}.json", now_unix_secs()));
        if !path.exists() {
            return path;
        }
        suffix += 1;
    }
}

fn quarantine(paths: &ArcPaths) -> Result<()> {
    let source = paths.skill_tracking_file();
    let target = unique_backup(paths, "corrupt");
    fs::rename(&source, &target)
        .map_err(|err| io_error("cannot isolate corrupt install metadata", &source, err))?;
    log::warn!(
        "install metadata was isolated at {}; existing targets remain unmanaged",
        target.display()
    );
    Ok(())
}
