use std::fs;
use std::path::{Path, PathBuf};

use log::warn;
use serde::{Deserialize, Serialize};

use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};
use crate::io::{atomic_write_bytes, now_unix_secs};
use crate::paths::ArcPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedGlobalSkillInstall {
    pub skill: String,
    pub agent: String,
    pub target_path: PathBuf,
    pub source_path: PathBuf,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackedGlobalSkillInstallRecord {
    skill: String,
    agent: String,
    source_path: String,
    #[serde(default)]
    source_fingerprint: String,
}

pub fn track_global_skill_install(
    paths: &ArcPaths,
    agent: &str,
    skill: &str,
    source_path: &Path,
) -> Result<()> {
    let mut records = load_tracking_records(paths)?;
    let record = TrackedGlobalSkillInstallRecord {
        skill: skill.to_string(),
        agent: agent.to_string(),
        source_path: source_path.display().to_string(),
        source_fingerprint: if source_path.exists() {
            fingerprint_path(source_path)?
        } else {
            String::new()
        },
    };
    records.retain(|item| !(item.agent == agent && item.skill == skill));
    records.push(record);
    records.sort_by(|a, b| (&a.agent, &a.skill).cmp(&(&b.agent, &b.skill)));
    write_tracking_records(paths, &records)
}

pub fn untrack_global_skill_install(paths: &ArcPaths, agent: &str, skill: &str) -> Result<()> {
    let mut records = load_tracking_records(paths)?;
    let before = records.len();
    records.retain(|item| !(item.agent == agent && item.skill == skill));
    if records.len() == before {
        return Ok(());
    }
    write_tracking_records(paths, &records)
}

pub fn untrack_global_skill_installs(paths: &ArcPaths, skill: &str) -> Result<()> {
    let mut records = load_tracking_records(paths)?;
    let before = records.len();
    records.retain(|item| item.skill != skill);
    if records.len() == before {
        return Ok(());
    }
    write_tracking_records(paths, &records)
}

pub fn list_tracked_global_skill_installs(
    paths: &ArcPaths,
    cache: &DetectCache,
) -> Result<Vec<TrackedGlobalSkillInstall>> {
    let records = load_tracking_records(paths)?;
    let mut tracked = Vec::new();

    for record in records {
        let Some(info) = cache.get_agent(&record.agent) else {
            continue;
        };
        let Some(root) = &info.root else {
            continue;
        };
        let Some(spec) = agent_spec(&record.agent) else {
            continue;
        };
        if !spec.supports_skills {
            continue;
        }
        let skills_dir = root.join(spec.skills_subdir);
        tracked.push(TrackedGlobalSkillInstall {
            target_path: skills_dir.join(&record.skill),
            source_path: PathBuf::from(record.source_path),
            source_fingerprint: record.source_fingerprint,
            skill: record.skill,
            agent: record.agent,
        });
    }

    Ok(tracked)
}

pub fn global_skill_target_needs_sync(
    target_path: &Path,
    strategy: SkillInstallStrategy,
    desired_source_path: &Path,
    desired_fingerprint: &str,
) -> Result<bool> {
    let meta = match fs::symlink_metadata(target_path) {
        Ok(meta) => meta,
        Err(_) => return Ok(true),
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
    let mut hasher = Fnv1a64::new();
    hash_path(path, path, &mut hasher)?;
    Ok(format!("{:016x}", hasher.finish()))
}

pub fn tracking_file_path(paths: &ArcPaths) -> PathBuf {
    paths.skill_tracking_file()
}

fn load_tracking_records(paths: &ArcPaths) -> Result<Vec<TrackedGlobalSkillInstallRecord>> {
    let path = tracking_file_path(paths);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = match fs::read(&path) {
        Ok(body) => body,
        Err(err) => return recover_from_tracking_read_failure(&path, err),
    };
    match serde_json::from_slice(&body) {
        Ok(records) => Ok(records),
        Err(err) => recover_from_tracking_parse_failure(&path, err),
    }
}

fn write_tracking_records(
    paths: &ArcPaths,
    records: &[TrackedGlobalSkillInstallRecord],
) -> Result<()> {
    let path = tracking_file_path(paths);
    if records.is_empty() {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| {
                ArcError::new(format!(
                    "failed to remove install metadata {}: {e}",
                    path.display()
                ))
            })?;
        }
        return Ok(());
    }
    let mut body = serde_json::to_vec_pretty(records)
        .map_err(|e| ArcError::new(format!("failed to serialize install metadata: {e}")))?;
    body.push(b'\n');
    atomic_write_bytes(&path, &body)
        .map_err(|e| ArcError::new(format!("failed to write install metadata: {e}")))
}

fn recover_from_tracking_read_failure(
    path: &Path,
    err: std::io::Error,
) -> Result<Vec<TrackedGlobalSkillInstallRecord>> {
    recover_corrupt_tracking_file(path, format!("read failed: {err}"))
}

fn recover_from_tracking_parse_failure(
    path: &Path,
    err: serde_json::Error,
) -> Result<Vec<TrackedGlobalSkillInstallRecord>> {
    recover_corrupt_tracking_file(path, format!("parse failed: {err}"))
}

fn recover_corrupt_tracking_file(
    path: &Path,
    reason: String,
) -> Result<Vec<TrackedGlobalSkillInstallRecord>> {
    let quarantined = quarantine_tracking_file(path)?;
    let warning = format!(
        "warning: install metadata {} was unreadable and moved to {}; continuing with empty tracking state ({reason})",
        path.display(),
        quarantined.display()
    );
    eprintln!("{warning}");
    warn!("{warning}");
    Ok(Vec::new())
}

fn quarantine_tracking_file(path: &Path) -> Result<PathBuf> {
    let Some(parent) = path.parent() else {
        return Err(ArcError::new(format!(
            "failed to isolate corrupt install metadata {}: missing parent directory",
            path.display()
        )));
    };
    let timestamp = now_unix_secs();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("installs");
    let extension = path.extension().and_then(|value| value.to_str());
    let mut quarantined = match extension {
        Some(ext) => parent.join(format!("{stem}.corrupt.{timestamp}.{ext}")),
        None => parent.join(format!("{stem}.corrupt.{timestamp}")),
    };
    let mut counter = 0usize;
    while quarantined.exists() {
        counter += 1;
        quarantined = match extension {
            Some(ext) => parent.join(format!("{stem}.corrupt.{timestamp}.{counter}.{ext}")),
            None => parent.join(format!("{stem}.corrupt.{timestamp}.{counter}")),
        };
    }
    fs::rename(path, &quarantined).map_err(|err| {
        ArcError::new(format!(
            "failed to isolate corrupt install metadata {} to {}: {err}",
            path.display(),
            quarantined.display()
        ))
    })?;
    Ok(quarantined)
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

fn hash_path(root: &Path, path: &Path, hasher: &mut Fnv1a64) -> Result<()> {
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
            .flatten()
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            hash_path(root, &entry.path(), hasher)?;
        }
        return Ok(());
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
