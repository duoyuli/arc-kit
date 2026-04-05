use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml::Value;

use crate::models::{ResourceInfo, ResourceKind};

pub fn scan_repo(repo_path: &Path, parser: &str, source_id: Option<&str>) -> Vec<ResourceInfo> {
    match parser {
        "auto" | "toml" => scan_auto(repo_path, source_id),
        "arc_native" => {
            let mut resources = scan_skills(repo_path, source_id);
            resources.extend(scan_arc_native(repo_path, source_id));
            resources
        }
        "skill_dir" => scan_skills(repo_path, source_id),
        _ => Vec::new(),
    }
}

pub fn scan_skills(repo_path: &Path, source_id: Option<&str>) -> Vec<ResourceInfo> {
    let source_id = source_id
        .map(str::to_string)
        .unwrap_or_else(|| source_id_for_repo(repo_path));
    let mut resources = Vec::new();
    for skillmd_path in find_files_named(repo_path, "SKILL.md") {
        let skill_dir = skillmd_path.parent().unwrap_or(repo_path);
        let skill_name = skill_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if skill_name.is_empty() {
            continue;
        }

        let frontmatter = parse_skillmd_frontmatter(&skillmd_path);
        let mut summary = frontmatter
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let arc_yaml = skill_dir.join("arc_cli.yaml");
        if arc_yaml.is_file()
            && let Some(meta) = parse_optional_skill_meta(&arc_yaml)
        {
            if let Some(name) = meta.get("name").and_then(Value::as_str)
                && name != skill_name
            {
                continue;
            }
            if let Some(meta_summary) = meta.get("summary").and_then(Value::as_str) {
                summary = meta_summary.to_string();
            }
        }

        if summary.is_empty() {
            summary = extract_summary_from_skillmd(&skillmd_path);
        }

        resources.push(ResourceInfo {
            id: format!("{source_id}/{skill_name}"),
            kind: ResourceKind::Skill,
            name: skill_name.to_string(),
            source_id: source_id.clone(),
            summary,
        });
    }
    resources
}

pub fn find_skill_directory(repo_path: &Path, name: &str) -> Option<PathBuf> {
    find_files_named(repo_path, "SKILL.md")
        .into_iter()
        .map(|path| path.parent().unwrap_or(repo_path).to_path_buf())
        .find(|path| path.file_name().and_then(|n| n.to_str()) == Some(name))
}

/// Extract a summary from a SKILL.md file: prefer frontmatter `description`, fall back to first prose line.
pub fn extract_skill_summary(skillmd_path: &Path) -> String {
    let frontmatter = parse_skillmd_frontmatter(skillmd_path);
    let summary = frontmatter
        .get("description")
        .and_then(serde_yaml::Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !summary.is_empty() {
        return summary;
    }
    extract_summary_from_skillmd(skillmd_path)
}

pub fn scan_arc_native(repo_path: &Path, source_id: Option<&str>) -> Vec<ResourceInfo> {
    find_files_named(repo_path, "arc_cli.yaml")
        .into_iter()
        .filter_map(|yaml_path| parse_arc_yaml(&yaml_path, source_id))
        .filter(|resource| resource.kind != ResourceKind::Skill)
        .collect()
}

fn scan_auto(repo_path: &Path, source_id: Option<&str>) -> Vec<ResourceInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for resource in scan_skills(repo_path, source_id)
        .into_iter()
        .chain(scan_arc_native(repo_path, source_id))
    {
        let key = (resource.id.clone(), resource.kind.clone());
        if seen.insert(key) {
            out.push(resource);
        }
    }
    out
}

fn source_id_for_repo(repo_path: &Path) -> String {
    for parent in repo_path.ancestors() {
        if parent.join(".git").is_dir() {
            return parent
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
        }
    }
    repo_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn parse_skillmd_frontmatter(path: &Path) -> serde_yaml::Mapping {
    let Ok(content) = fs::read_to_string(path) else {
        return serde_yaml::Mapping::new();
    };
    if !content.starts_with("---") {
        return serde_yaml::Mapping::new();
    }
    let Some(end) = content[3..].find("\n---") else {
        return serde_yaml::Mapping::new();
    };
    let raw = &content[3..(3 + end)];
    serde_yaml::from_str(raw).unwrap_or_default()
}

fn parse_optional_skill_meta(path: &Path) -> Option<serde_yaml::Mapping> {
    let raw = fs::read_to_string(path).ok()?;
    let value: Value = serde_yaml::from_str(&raw).ok()?;
    let Value::Mapping(map) = value else {
        return None;
    };
    if map
        .get(Value::String("kind".to_string()))
        .and_then(Value::as_str)
        != Some("skill")
    {
        return None;
    }
    Some(map)
}

fn extract_summary_from_skillmd(path: &Path) -> String {
    let Ok(content) = fs::read_to_string(path) else {
        return String::new();
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "---" || trimmed.contains(':') {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        return trimmed.to_string();
    }
    String::new()
}

fn parse_arc_yaml(path: &Path, source_id: Option<&str>) -> Option<ResourceInfo> {
    let raw = fs::read_to_string(path).ok()?;
    let value: Value = serde_yaml::from_str(&raw).ok()?;
    let kind = value.get("kind")?.as_str()?.parse().ok()?;
    let name = value.get("name")?.as_str()?.to_string();
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let source_id = source_id
        .map(str::to_string)
        .or_else(|| {
            path.parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    Some(ResourceInfo {
        id: format!("{source_id}/{name}"),
        kind,
        name,
        source_id,
        summary,
    })
}

const MAX_SCAN_DEPTH: usize = 10;

fn find_files_named(root: &Path, needle: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((path, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && depth < MAX_SCAN_DEPTH {
                stack.push((path, depth + 1));
            } else if path.file_name().and_then(|s| s.to_str()) == Some(needle) {
                out.push(path);
            }
        }
    }
    out
}
