use std::fs;
use std::path::Path;

use include_dir::{Dir, include_dir};

use crate::io::atomic_write_bytes;
use crate::market::scanner::extract_skill_summary;
use crate::models::{SkillEntry, SkillOrigin};

// 产品资源可以为空；测试夹具通过私有扫描入口单独注入，避免随二进制分发。
static BUILTIN_SKILL_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../built-in/skill");

/// 只读取内置技能及已有缓存中的摘要，不创建缓存目录。
pub fn list_builtin_skills(cache_dir: &Path) -> Vec<SkillEntry> {
    list_embedded_skills(&BUILTIN_SKILL_DIR, cache_dir)
}

fn list_embedded_skills(source: &Dir<'_>, cache_dir: &Path) -> Vec<SkillEntry> {
    let mut entries = Vec::new();
    for dir in source.dirs() {
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

/// 将内置技能解包到 `cache_dir/<name>/`，成功时返回目标路径。
pub fn materialize(cache_dir: &Path, name: &str) -> std::io::Result<std::path::PathBuf> {
    materialize_embedded_skill(&BUILTIN_SKILL_DIR, cache_dir, name)
}

fn materialize_embedded_skill(
    source: &Dir<'_>,
    cache_dir: &Path,
    name: &str,
) -> std::io::Result<std::path::PathBuf> {
    let Some(dir) = source.get_dir(name) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    // 正向扫描和解包测试使用专用资源，不依赖产品是否附带某个具体技能。
    static FIXTURES: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/builtin_skills");

    #[test]
    fn embedded_scan_reads_summary_without_creating_cache() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");

        let entries = list_embedded_skills(&FIXTURES, &cache);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "sample-builtin");
        assert_eq!(entries[0].origin, SkillOrigin::BuiltIn);
        assert_eq!(entries[0].summary, "用于验证内置技能扫描与解包的测试夹具。");
        assert_eq!(entries[0].source_path, cache.join("sample-builtin"));
        assert!(!cache.exists());
    }

    #[test]
    fn embedded_materialization_extracts_nested_resources_and_refreshes_files() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        let dest = materialize_embedded_skill(&FIXTURES, &cache, "sample-builtin").unwrap();

        assert_eq!(dest, cache.join("sample-builtin"));
        assert_eq!(
            fs::read(dest.join("SKILL.md")).unwrap(),
            FIXTURES
                .get_file("sample-builtin/SKILL.md")
                .unwrap()
                .contents()
        );
        assert_eq!(
            fs::read(dest.join("references/details.md")).unwrap(),
            FIXTURES
                .get_file("sample-builtin/references/details.md")
                .unwrap()
                .contents()
        );

        // 重新物化应恢复嵌入内容，同时保留缓存中无关的文件。
        fs::write(dest.join("references/details.md"), "旧内容").unwrap();
        fs::write(dest.join("local-note.txt"), "保留").unwrap();
        materialize_embedded_skill(&FIXTURES, &cache, "sample-builtin").unwrap();
        assert_eq!(
            fs::read(dest.join("references/details.md")).unwrap(),
            FIXTURES
                .get_file("sample-builtin/references/details.md")
                .unwrap()
                .contents()
        );
        assert_eq!(
            fs::read_to_string(dest.join("local-note.txt")).unwrap(),
            "保留"
        );
    }

    #[test]
    fn embedded_scan_prefers_existing_cached_summary() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("cache");
        let dest = materialize_embedded_skill(&FIXTURES, &cache, "sample-builtin").unwrap();
        fs::write(
            dest.join("SKILL.md"),
            "---\nname: sample-builtin\ndescription: 缓存中的摘要\n---\n",
        )
        .unwrap();

        let entries = list_embedded_skills(&FIXTURES, &cache);

        assert_eq!(entries[0].summary, "缓存中的摘要");
    }
}
