use std::fs;

use arc_core::detect::DetectCache;
use arc_core::models::SkillOrigin;
use arc_core::paths::ArcPaths;
use arc_core::skill::SkillRegistry;

#[test]
fn registry_list_all_includes_builtin_skills() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);

    let registry = SkillRegistry::new(paths, cache);
    let skills = registry.list_all();
    assert!(
        skills.iter().any(|s| s.name == "arc-cli-usage"),
        "should include built-in arc-cli-usage skill"
    );
}

#[test]
fn registry_list_all_includes_local_skills() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);

    let local_dir = paths.local_skills_dir();
    let my_skill = local_dir.join("my-local-skill");
    fs::create_dir_all(&my_skill).unwrap();
    fs::write(my_skill.join("SKILL.md"), "Local skill body").unwrap();

    let registry = SkillRegistry::new(paths, cache);
    let skills = registry.list_all();
    let found = skills.iter().find(|s| s.name == "my-local-skill");
    assert!(found.is_some());
    assert_eq!(found.unwrap().origin, SkillOrigin::Local);
}

#[test]
fn registry_local_overrides_builtin_on_same_name() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);

    let local_dir = paths.local_skills_dir();
    let local_skill = local_dir.join("arc-cli-usage");
    fs::create_dir_all(&local_skill).unwrap();
    fs::write(local_skill.join("SKILL.md"), "Local override").unwrap();

    let registry = SkillRegistry::new(paths, cache);
    let skills = registry.list_all();
    let entry = skills.iter().find(|s| s.name == "arc-cli-usage").unwrap();
    assert_eq!(entry.origin, SkillOrigin::Local);
}

#[test]
fn registry_find_returns_highest_priority() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);

    let local_dir = paths.local_skills_dir();
    let local_skill = local_dir.join("arc-cli-usage");
    fs::create_dir_all(&local_skill).unwrap();
    fs::write(local_skill.join("SKILL.md"), "Local override").unwrap();

    let registry = SkillRegistry::new(paths, cache);
    let entry = registry.find("arc-cli-usage").unwrap();
    assert_eq!(entry.origin, SkillOrigin::Local);
}

#[test]
fn registry_find_returns_none_for_unknown() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);
    let registry = SkillRegistry::new(paths, cache);
    assert!(registry.find("nonexistent-xyz").is_none());
}

#[test]
fn registry_resolve_source_path_materializes_builtin() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let cache = DetectCache::new(&paths);
    let registry = SkillRegistry::new(paths.clone(), cache);

    let entry = registry.find("arc-cli-usage").unwrap();
    let source_path = registry.resolve_source_path(&entry).unwrap();
    assert!(source_path.join("SKILL.md").is_file());
    assert_eq!(source_path, paths.builtin_cache_dir().join("arc-cli-usage"));
}
