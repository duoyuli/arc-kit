use std::fs;
use std::path::{Path, PathBuf};

use ring::digest::{Context, SHA256};

use super::install::{
    InstallOperation, InstallRecord, InstallScope, InstallStore, candidate_destinations,
    inspect_install_ledger, inspect_owned,
};
use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::paths::ArcPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedGlobalSkillInstall {
    pub skill: String,
    pub agent: String,
    pub target_path: PathBuf,
    pub source_path: PathBuf,
    pub source_fingerprint: String,
    pub target_fingerprint: Option<String>,
    pub strategy: SkillInstallStrategy,
}

/// Record an already materialized global target after verifying its source.
pub fn track_global_skill_install(
    paths: &ArcPaths,
    agent: &str,
    skill: &str,
    source_path: &Path,
) -> Result<()> {
    let mut store = InstallStore::open(paths)?;
    store.recover(InstallScope::Global, None, &[agent.to_string()])?;
    let target =
        candidate_destinations(paths, InstallScope::Global, None, agent, skill)?[0].clone();
    let strategy = agent_spec(agent)
        .ok_or_else(|| ArcError::new("unknown skill agent"))?
        .skill_install_strategy;
    let mut record = InstallRecord::new(
        InstallScope::Global,
        None,
        agent,
        skill,
        source_path,
        &target,
        strategy,
    )?;
    if strategy == SkillInstallStrategy::Copy {
        let fingerprint = fingerprint_path(&target)?;
        if fingerprint != record.source_fingerprint {
            return Err(ArcError::new("installed copy does not match its source"));
        }
        record.target_fingerprint = Some(fingerprint);
    }
    if !inspect_owned(paths, &record)? {
        return Err(ArcError::new("cannot track a missing installation"));
    }
    let previous = store.ledger.record(&target).cloned();
    store.execute(InstallOperation::put(record, previous, true))
}

pub fn untrack_global_skill_install(paths: &ArcPaths, agent: &str, skill: &str) -> Result<()> {
    untrack_matching(paths, Some(agent), skill)
}

pub fn untrack_global_skill_installs(paths: &ArcPaths, skill: &str) -> Result<()> {
    untrack_matching(paths, None, skill)
}

fn untrack_matching(paths: &ArcPaths, agent: Option<&str>, skill: &str) -> Result<()> {
    let mut store = InstallStore::open(paths)?;
    let records: Vec<_> = store
        .ledger
        .in_scope(InstallScope::Global, None)
        .filter(|record| record.skill == skill && agent.is_none_or(|agent| record.agent == agent))
        .cloned()
        .collect();
    for record in &records {
        if inspect_owned(paths, record)? {
            return Err(ArcError::new(format!(
                "cannot forget an existing installation: {}",
                record.target_path.display()
            )));
        }
    }
    for record in records {
        let mut operation = InstallOperation::remove(record);
        operation.metadata_only = true;
        store.execute(operation)?;
    }
    Ok(())
}

pub fn list_tracked_global_skill_installs(
    paths: &ArcPaths,
    _cache: &DetectCache,
) -> Result<Vec<TrackedGlobalSkillInstall>> {
    Ok(inspect_install_ledger(paths)?
        .installs
        .into_iter()
        .filter(|record| record.scope == InstallScope::Global)
        .map(|record| TrackedGlobalSkillInstall {
            skill: record.skill,
            agent: record.agent,
            target_path: record.target_path,
            source_path: record.source_path,
            source_fingerprint: record.source_fingerprint,
            target_fingerprint: record.target_fingerprint,
            strategy: record.strategy,
        })
        .collect())
}

pub fn global_skill_target_needs_sync(
    target_path: &Path,
    strategy: SkillInstallStrategy,
    desired_source_path: &Path,
    desired_fingerprint: &str,
) -> Result<bool> {
    let meta = match fs::symlink_metadata(target_path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(err) => {
            return Err(ArcError::new(format!(
                "failed to inspect {}: {err}",
                target_path.display()
            )));
        }
    };

    match strategy {
        SkillInstallStrategy::Symlink => {
            if !meta.file_type().is_symlink() {
                return Ok(true);
            }
            let actual = fs::read_link(target_path).map_err(|e| {
                ArcError::new(format!(
                    "failed to read skill symlink {}: {e}",
                    target_path.display()
                ))
            })?;
            let actual = absolutize_link_target(target_path, &actual);
            Ok(actual != desired_source_path)
        }
        SkillInstallStrategy::Copy => {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Ok(true);
            }
            Ok(fingerprint_path(target_path)? != desired_fingerprint)
        }
    }
}

pub fn fingerprint_path(path: &Path) -> Result<String> {
    let mut hasher = Context::new(&SHA256);
    hash_path(path, path, &mut hasher)?;
    let digest = hasher.finish();
    let hex: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(format!("sha256:{hex}"))
}

// 历史指纹仅用于迁移时核对旧记录，不能独立证明副本归属。
pub(crate) fn legacy_fingerprint_path(path: &Path) -> Result<String> {
    let mut hasher = Fnv1a64::new();
    hash_legacy_path(path, path, &mut hasher)?;
    Ok(format!("{:016x}", hasher.finish()))
}

pub fn tracking_file_path(paths: &ArcPaths) -> PathBuf {
    paths.skill_tracking_file()
}

fn absolutize_link_target(link_path: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }
    link_path
        .parent()
        .map(|parent| parent.join(target))
        .unwrap_or_else(|| target.to_path_buf())
}

fn hash_path(root: &Path, path: &Path, hasher: &mut Context) -> Result<()> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| ArcError::new(format!("failed to stat {}: {e}", path.display())))?;
    // 路径、类型和内容分别编码长度，避免文件内容与下一条路径拼接成相同输入。
    hash_field(
        hasher,
        path.strip_prefix(root)
            .unwrap_or(path)
            .as_os_str()
            .as_encoded_bytes(),
    );
    if meta.file_type().is_symlink() {
        hash_field(hasher, b"link");
        let target = fs::read_link(path)
            .map_err(|e| ArcError::new(format!("failed to read link {}: {e}", path.display())))?;
        hash_field(hasher, target.as_os_str().as_encoded_bytes());
    } else if meta.is_dir() {
        hash_field(hasher, b"dir");
        let mut entries = fs::read_dir(path)
            .and_then(|entries| entries.collect::<std::io::Result<Vec<_>>>())
            .map_err(|e| ArcError::new(format!("failed to read dir {}: {e}", path.display())))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            hash_path(root, &entry.path(), hasher)?;
        }
    } else if meta.is_file() {
        hash_field(hasher, b"file");
        let body = fs::read(path)
            .map_err(|e| ArcError::new(format!("failed to read file {}: {e}", path.display())))?;
        hash_field(hasher, &body);
    } else {
        return Err(ArcError::new(format!(
            "unsupported file type in skill contents: {}",
            path.display()
        )));
    }
    Ok(())
}

fn hash_field(hasher: &mut Context, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn hash_legacy_path(root: &Path, path: &Path, hasher: &mut Fnv1a64) -> Result<()> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| ArcError::new(format!("failed to stat {}: {e}", path.display())))?;
    let rel = path.strip_prefix(root).unwrap_or(path);
    hasher.update(rel.to_string_lossy().as_bytes());

    if meta.file_type().is_symlink() {
        hasher.update(b"link");
        let target = fs::read_link(path)
            .map_err(|e| ArcError::new(format!("failed to read link {}: {e}", path.display())))?;
        hasher.update(target.to_string_lossy().as_bytes());
        return Ok(());
    }

    if meta.is_dir() {
        hasher.update(b"dir");
        let mut entries: Vec<_> = fs::read_dir(path)
            .map_err(|e| ArcError::new(format!("failed to read dir {}: {e}", path.display())))?
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| {
                ArcError::new(format!("failed to read entry in {}: {e}", path.display()))
            })?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            hash_legacy_path(root, &entry.path(), hasher)?;
        }
        return Ok(());
    }

    if !meta.is_file() {
        return Err(ArcError::new(format!(
            "unsupported file type in skill contents: {}",
            path.display()
        )));
    }
    hasher.update(b"file");
    let body = fs::read(path)
        .map_err(|e| ArcError::new(format!("failed to read file {}: {e}", path.display())))?;
    hasher.update(&body);
    Ok(())
}

struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self {
            state: Self::OFFSET_BASIS,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.state
    }
}
