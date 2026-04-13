use std::fs;
use std::path::Path;

use include_dir::{Dir, include_dir};

use crate::io::atomic_write_bytes;
use crate::market::scanner::extract_skill_summary;
use crate::models::{SkillEntry, SkillOrigin};

static BUILTIN_SKILL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../built-in/skill");

pub fn list_builtin_skills(cache_dir: &Path) -> Vec<SkillEntry> {
    let mut entries = Vec::new();
    for dir in BUILTIN_SKILL_DIR.dirs() {
        let name = dir
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let has_skill_md = dir
            .files()
            .any(|f| f.path().file_name().and_then(|s| s.to_str()) == Some("SKILL.md"));
        if !has_skill_md {
            continue;
        }
        let dest = cache_dir.join(&name);
        let summary = if dest.join("SKILL.md").is_file() {
            extract_skill_summary(&dest.join("SKILL.md"))
        } else {
            embedded_summary(dir)
        };
        entries.push(SkillEntry {
            name,
            origin: SkillOrigin::BuiltIn,
            summary,
            source_path: dest,
            installed_targets: Vec::new(),
            market_repo: None,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Extract the cached built-in skill files to `cache_dir/<name>/`.
/// Returns the destination path on success.
pub fn materialize(cache_dir: &Path, name: &str) -> std::io::Result<std::path::PathBuf> {
    let Some(dir) = BUILTIN_SKILL_DIR.get_dir(name) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("built-in skill '{name}' not found"),
        ));
    };
    let dest = cache_dir.join(name);
    fs::create_dir_all(&dest)?;
    extract_dir(dir, &dest)?;
    Ok(dest)
}

fn extract_dir(dir: &Dir, dest: &Path) -> std::io::Result<()> {
    for file in dir.files() {
        let rel = file.path().strip_prefix(dir.path()).unwrap_or(file.path());
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write_bytes(&target, file.contents())?;
    }
    for sub in dir.dirs() {
        let rel = sub.path().strip_prefix(dir.path()).unwrap_or(sub.path());
        extract_dir(sub, &dest.join(rel))?;
    }
    Ok(())
}

fn embedded_summary(dir: &Dir) -> String {
    let Some(file) = dir
        .files()
        .find(|f| f.path().file_name().and_then(|s| s.to_str()) == Some("SKILL.md"))
    else {
        return String::new();
    };
    let content = std::str::from_utf8(file.contents()).unwrap_or_default();
    parse_embedded_description(content)
}

fn parse_embedded_description(content: &str) -> String {
    if !content.starts_with("---") {
        return first_prose_line(content);
    }
    let Some(end) = content[3..].find("\n---") else {
        return first_prose_line(content);
    };
    let raw = &content[3..(3 + end)];
    let mapping: serde_yaml::Mapping = serde_yaml::from_str(raw).unwrap_or_default();
    if let Some(desc) = mapping
        .get("description")
        .and_then(serde_yaml::Value::as_str)
    {
        return desc.to_string();
    }
    first_prose_line(content)
}

fn first_prose_line(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed == "---"
            || trimmed.contains(':')
            || trimmed.starts_with('#')
        {
            continue;
        }
        return trimmed.to_string();
    }
    String::new()
}
