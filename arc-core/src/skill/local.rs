use std::fs;
use std::path::Path;

use crate::market::scanner::extract_skill_summary;
use crate::models::{SkillEntry, SkillOrigin};

pub fn scan_local_skills(skills_dir: &Path) -> Vec<SkillEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(skills_dir) else {
        return entries;
    };
    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let summary = extract_skill_summary(&skill_md);
        entries.push(SkillEntry {
            name,
            origin: SkillOrigin::Local,
            summary,
            source_path: path,
            installed_targets: Vec::new(),
            market_repo: None,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}
