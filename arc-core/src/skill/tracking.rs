use std::fs;
use std::path::{Path, PathBuf};

use log::warn;
use serde::{Deserialize, Serialize};

use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::detect::DetectCache;
use crate::error::{ArcError, Result};

const TRACKING_PREFIX: &str = ".arc-skill-install.";
const TRACKING_SUFFIX: &str = ".json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedGlobalSkillInstall {
    pub skill: String,
    pub agent: String,
    pub skills_dir: PathBuf,
    pub target_path: PathBuf,
    pub metadata_path: PathBuf,
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
    skills_dir: &Path,
    agent: &str,
    skill: &str,
    source_path: &Path,
) -> Result<()> {
    fs::create_dir_all(skills_dir)
        .map_err(|e| ArcError::new(format!("failed to create skills dir: {e}")))?;
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
    let body = serde_json::to_vec_pretty(&record)
        .map_err(|e| ArcError::new(format!("failed to serialize install metadata: {e}")))?;
    fs::write(metadata_path(skills_dir, skill), body)
        .map_err(|e| ArcError::new(format!("failed to write install metadata: {e}")))?;
    Ok(())
}

pub fn untrack_global_skill_install(skills_dir: &Path, skill: &str) -> Result<()> {
    let path = metadata_path(skills_dir, skill);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|e| ArcError::new(format!("failed to remove install metadata: {e}")))?;
    Ok(())
}

pub fn list_tracked_global_skill_installs(cache: &DetectCache) -> Vec<TrackedGlobalSkillInstall> {
    let mut tracked = Vec::new();

    for (agent_id, info) in cache.detected_agents() {
        let Some(root) = &info.root else {
            continue;
        };
        let Some(spec) = agent_spec(agent_id) else {
            continue;
        };
        if !spec.supports_skills {
            continue;
        }
        let skills_dir = root.join(spec.skills_subdir);
        let Ok(entries) = fs::read_dir(&skills_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !is_tracking_file_name(&file_name) {
                continue;
            }
            match load_tracking_record(&path) {
                Ok(record) => tracked.push(TrackedGlobalSkillInstall {
                    target_path: skills_dir.join(&record.skill),
                    metadata_path: path,
                    skills_dir: skills_dir.clone(),
                    source_path: PathBuf::from(record.source_path),
                    source_fingerprint: record.source_fingerprint,
                    skill: record.skill,
                    agent: record.agent,
                }),
                Err(err) => warn!(
                    "ignoring invalid skill install metadata {}: {}",
                    path.display(),
                    err
                ),
            }
        }
    }

    tracked
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

pub fn is_arc_tracking_file_name(name: &str) -> bool {
    is_tracking_file_name(name)
}

pub fn tracking_file_path(skills_dir: &Path, skill: &str) -> PathBuf {
    metadata_path(skills_dir, skill)
}

fn load_tracking_record(path: &Path) -> Result<TrackedGlobalSkillInstallRecord> {
    let body = fs::read(path).map_err(|e| {
        ArcError::new(format!(
            "failed to read install metadata {}: {e}",
            path.display()
        ))
    })?;
    serde_json::from_slice(&body).map_err(|e| {
        ArcError::new(format!(
            "failed to parse install metadata {}: {e}",
            path.display()
        ))
    })
}

fn metadata_path(skills_dir: &Path, skill: &str) -> PathBuf {
    skills_dir.join(format!("{TRACKING_PREFIX}{skill}{TRACKING_SUFFIX}"))
}

fn is_tracking_file_name(name: &str) -> bool {
    name.starts_with(TRACKING_PREFIX) && name.ends_with(TRACKING_SUFFIX)
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
