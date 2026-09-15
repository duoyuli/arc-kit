use std::fs;

use arc_core::detect::DetectCache;
use arc_core::market::catalog::CatalogManager;
use arc_core::models::{ResourceInfo, ResourceKind, SkillEntry, SkillOrigin};
use arc_core::paths::ArcPaths;
use arc_core::skill::SkillRegistry;

// 所有来源与检测结果都由测试构造，避免依赖本机 agent 或产品附带的技能。
fn setup() -> (tempfile::TempDir, ArcPaths, SkillRegistry) {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let registry = SkillRegistry::new(paths.clone(), DetectCache::from_map(Default::default()));
    (temp, paths, registry)
}

fn add_local(paths: &ArcPaths, name: &str) -> std::path::PathBuf {
    let source = paths.local_skills_dir().join(name);
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# 本地测试技能\n").unwrap();
    source
}

fn add_market(paths: &ArcPaths, name: &str) {
    CatalogManager::new(paths.clone())
        .rebuild(&[ResourceInfo {
            id: format!("fixture-market/{name}"),
            kind: ResourceKind::Skill,
            name: name.to_string(),
            source_id: "fixture-market".to_string(),
            summary: "市场测试技能".to_string(),
        }])
        .unwrap();
}

#[test]
fn registry_lists_local_and_market_with_empty_builtin_source() {
    let (_temp, paths, registry) = setup();
    add_local(&paths, "local-skill");
    add_market(&paths, "market-skill");

    let skills = registry.list_all();

    assert_eq!(skills.len(), 2);
    assert_eq!(skills[0].name, "local-skill");
    assert_eq!(skills[0].origin, SkillOrigin::Local);
    assert_eq!(skills[1].name, "market-skill");
    assert_eq!(
        skills[1].origin,
        SkillOrigin::Market {
            source_id: "fixture-market".to_string()
        }
    );
}

#[test]
fn registry_local_overrides_market_in_list_and_find() {
    let (_temp, paths, registry) = setup();
    add_market(&paths, "shared-skill");
    let local = add_local(&paths, "shared-skill");

    // 使用真实的同名市场项与本地项验证优先级，不以空内置目录制造虚假冲突。
    let skills = registry.list_all();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].origin, SkillOrigin::Local);
    assert_eq!(skills[0].source_path, local);
    let entry = registry.find("shared-skill").unwrap();
    assert_eq!(entry.origin, SkillOrigin::Local);
    assert_eq!(entry.source_path, local);
}

#[test]
fn registry_find_returns_none_for_unknown() {
    let (_temp, _paths, registry) = setup();
    assert!(registry.find("nonexistent-xyz").is_none());
}

#[test]
fn registry_resolves_local_source_without_creating_builtin_cache() {
    let (_temp, paths, registry) = setup();
    let source = add_local(&paths, "local-skill");
    let entry = registry.find("local-skill").unwrap();

    assert_eq!(registry.resolve_source_path(&entry).unwrap(), source);
    assert_eq!(
        registry.resolve_source_path_readonly(&entry).unwrap(),
        source
    );
    assert!(!paths.builtin_cache_dir().exists());
}

#[test]
fn readonly_builtin_resolution_requires_existing_cache_without_materializing() {
    let (_temp, paths, registry) = setup();
    let source = paths.builtin_cache_dir().join("cached-builtin");
    // 直接构造内置条目，独立验证只读解析不会触发产品资源的物化。
    let entry = SkillEntry {
        name: "cached-builtin".to_string(),
        origin: SkillOrigin::BuiltIn,
        summary: String::new(),
        source_path: source.clone(),
        installed_targets: Vec::new(),
        market_repo: None,
    };

    assert!(registry.resolve_source_path_readonly(&entry).is_err());
    assert!(!paths.builtin_cache_dir().exists());
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# 已有缓存\n").unwrap();
    assert_eq!(
        registry.resolve_source_path_readonly(&entry).unwrap(),
        source
    );
}
