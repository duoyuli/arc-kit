use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::agent::{SkillInstallStrategy, agent_spec};
use crate::error::{ArcError, Result};
use crate::paths::ArcPaths;
use crate::skill::tracking::fingerprint_path;

use super::{InstallRecord, InstallScope};

pub(crate) fn io_error(action: &str, path: &Path, err: impl std::fmt::Display) -> ArcError {
    ArcError::new(format!("{action} {}: {err}", path.display()))
}

pub(crate) fn sync_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|err| io_error("cannot sync installation directory", path, err))
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with(".arc-")
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err(ArcError::new(format!("invalid skill name '{name}'")));
    }
    Ok(())
}

pub(crate) fn absolute_lexical(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| ArcError::new(err.to_string()))?
            .join(path)
    };
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    Ok(result)
}

fn resolve_existing_parent(path: &Path) -> Result<PathBuf> {
    let mut probe = absolute_lexical(path)?;
    let mut suffix = Vec::new();
    loop {
        match fs::canonicalize(&probe) {
            Ok(mut existing) => {
                for component in suffix.iter().rev() {
                    existing.push(component);
                }
                return Ok(existing);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                match fs::symlink_metadata(&probe) {
                    Ok(_) => return Err(io_error("cannot resolve existing parent", &probe, err)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(io_error("cannot inspect parent", &probe, error)),
                }
                let component = probe
                    .file_name()
                    .ok_or_else(|| io_error("cannot resolve path", path, &err))?
                    .to_os_string();
                suffix.push(component);
                if !probe.pop() {
                    return Err(io_error("cannot resolve path", path, err));
                }
            }
            Err(err) => return Err(io_error("cannot resolve path", &probe, err)),
        }
    }
}

pub fn normalize_root(path: &Path) -> Result<PathBuf> {
    let root =
        fs::canonicalize(path).map_err(|err| io_error("project root unavailable", path, err))?;
    if !root.is_dir() {
        return Err(ArcError::new(format!(
            "project root is not a directory: {}",
            root.display()
        )));
    }
    fs::read_dir(&root).map_err(|err| io_error("project root unavailable", &root, err))?;
    Ok(root)
}

/// 解析父目录别名，但不跟随安装条目自身的软链接。
pub fn normalize_destination(path: &Path) -> Result<PathBuf> {
    let absolute = absolute_lexical(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| ArcError::new("installation destination has no parent"))?;
    let name = absolute
        .file_name()
        .ok_or_else(|| ArcError::new("installation destination has no name"))?;
    Ok(resolve_existing_parent(parent)?.join(name))
}

pub(crate) fn candidate_destinations(
    paths: &ArcPaths,
    scope: InstallScope,
    project_root: Option<&Path>,
    agent: &str,
    skill: &str,
) -> Result<Vec<PathBuf>> {
    validate_name(skill)?;
    let spec = agent_spec(agent)
        .ok_or_else(|| ArcError::new(format!("unknown installation layout for agent '{agent}'")))?;
    let (root, directories) = match scope {
        InstallScope::Global => {
            let root = resolve_existing_parent(&paths.user_home().join(spec.home_parts.join("/")))?;
            (root, vec![PathBuf::from(spec.skills_subdir)])
        }
        InstallScope::Project => {
            if !spec.supports_project_skills {
                return Err(ArcError::new(format!(
                    "agent '{agent}' does not support project skills"
                )));
            }
            let root = normalize_root(
                project_root
                    .ok_or_else(|| ArcError::new("project installation has no project root"))?,
            )?;
            let mut dirs = vec![PathBuf::from(spec.project_skills_parts.join("/"))];
            if agent == "cursor" {
                dirs.push(PathBuf::from(".cursor/skills-cursor"));
            }
            (root, dirs)
        }
    };
    let mut result = Vec::new();
    for directory in directories {
        let target = normalize_destination(&root.join(directory).join(skill))?;
        if !target.starts_with(&root) || target == root {
            return Err(ArcError::new(format!(
                "installation destination escapes its scope: {}",
                target.display()
            )));
        }
        if !result.contains(&target) {
            result.push(target);
        }
    }
    Ok(result)
}

pub(crate) fn validate_destination(paths: &ArcPaths, record: &InstallRecord) -> Result<()> {
    if !record.target_path.is_absolute()
        || !record.source_path.is_absolute()
        || (record.scope == InstallScope::Project) != record.project_root.is_some()
    {
        return Err(ArcError::new(
            "invalid installation scope or non-absolute path",
        ));
    }
    if record
        .target_path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(ArcError::new(
            "installation destination contains parent traversal",
        ));
    }
    let candidates = candidate_destinations(
        paths,
        record.scope,
        record.project_root.as_deref(),
        &record.agent,
        &record.skill,
    )?;
    if !candidates.contains(&record.target_path) {
        return Err(ArcError::new(format!(
            "unrecognized installation destination: {}",
            record.target_path.display()
        )));
    }
    Ok(())
}

pub(crate) fn link_destination(path: &Path) -> Result<PathBuf> {
    let target =
        fs::read_link(path).map_err(|err| io_error("cannot read installed symlink", path, err))?;
    absolute_lexical(&if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or(Path::new("/")).join(target)
    })
}

/// 返回真表示目标存在且归属匹配；目标缺失与读取失败分别处理。
pub(crate) fn inspect_owned(paths: &ArcPaths, record: &InstallRecord) -> Result<bool> {
    validate_destination(paths, record)?;
    let metadata = match fs::symlink_metadata(&record.target_path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(io_error(
                "cannot inspect installed target",
                &record.target_path,
                err,
            ));
        }
    };
    let owned = match record.strategy {
        SkillInstallStrategy::Symlink => {
            metadata.file_type().is_symlink()
                && link_destination(&record.target_path)? == record.source_path
        }
        SkillInstallStrategy::Copy => {
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && fingerprint_path(&record.target_path)? == record.copy_fingerprint()?
        }
    };
    if !owned {
        return Err(ArcError::new(format!(
            "installed target was modified or replaced: {}",
            record.target_path.display()
        )));
    }
    Ok(true)
}
