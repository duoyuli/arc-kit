use std::fs;

use arc_core::skill::builtin;

#[test]
fn current_bundle_has_no_builtin_skills_and_does_not_create_cache() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");

    // 当前产品没有附带技能，正向解包由模块内专用夹具覆盖。
    let entries = builtin::list_builtin_skills(&cache_dir);

    assert!(entries.is_empty());
    assert!(!cache_dir.exists());
}

#[test]
fn stale_cache_does_not_register_or_restore_an_unbundled_skill() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");
    let stale = cache_dir.join("unbundled-skill");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("SKILL.md"), "# 历史缓存\n").unwrap();

    // 缓存不能单独成为内置来源，也不能让已移除资源重新被物化。
    assert!(builtin::list_builtin_skills(&cache_dir).is_empty());
    let error = builtin::materialize(&cache_dir, "unbundled-skill").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        fs::read_to_string(stale.join("SKILL.md")).unwrap(),
        "# 历史缓存\n"
    );
}

#[test]
fn materialize_returns_not_found_without_creating_cache_for_unknown_skill() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");

    let error = builtin::materialize(&cache_dir, "nonexistent").unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(!cache_dir.exists());
}
