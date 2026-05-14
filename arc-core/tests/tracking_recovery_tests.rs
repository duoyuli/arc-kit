use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use arc_core::detect::DetectCache;
use arc_core::engine::InstallEngine;
use arc_core::paths::ArcPaths;
use arc_core::skill::SkillRegistry;
use arc_core::skill::tracking::{
    list_tracked_global_skill_installs, track_global_skill_install, untrack_global_skill_install,
};

fn empty_cache() -> DetectCache {
    DetectCache::from_map(BTreeMap::new())
}

fn write_corrupt_tracking_file(paths: &ArcPaths) {
    let file = paths.skill_tracking_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, "{not valid json").unwrap();
}

fn find_corrupt_tracking_file(paths: &ArcPaths) -> PathBuf {
    let tracking_file = paths.skill_tracking_file();
    let dir = tracking_file.parent().unwrap();
    let mut matches = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("installs.corrupt.") && name.ends_with(".json")
                })
        })
        .collect::<Vec<_>>();
    matches.sort();
    assert_eq!(matches.len(), 1, "expected one quarantined tracking file");
    matches.remove(0)
}

#[test]
fn list_tracked_global_skill_installs_quarantines_corrupt_file() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    write_corrupt_tracking_file(&paths);

    let installs = list_tracked_global_skill_installs(&paths, &empty_cache()).unwrap();

    assert!(installs.is_empty());
    assert!(!paths.skill_tracking_file().exists());
    let quarantined = find_corrupt_tracking_file(&paths);
    assert_eq!(fs::read_to_string(quarantined).unwrap(), "{not valid json");
}

#[test]
fn track_global_skill_install_recovers_from_corrupt_file() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    write_corrupt_tracking_file(&paths);

    let source = temp.path().join("source").join("demo");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# demo\n").unwrap();

    track_global_skill_install(&paths, "claude", "demo", &source).unwrap();

    let body = fs::read_to_string(paths.skill_tracking_file()).unwrap();
    assert!(body.contains("\"agent\": \"claude\""));
    assert!(body.contains("\"skill\": \"demo\""));
    assert!(find_corrupt_tracking_file(&paths).exists());
}

#[test]
fn untrack_global_skill_install_recovers_from_corrupt_file() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    write_corrupt_tracking_file(&paths);

    untrack_global_skill_install(&paths, "claude", "demo").unwrap();

    assert!(!paths.skill_tracking_file().exists());
    assert!(find_corrupt_tracking_file(&paths).exists());
}

#[test]
fn global_skill_sync_recovers_from_corrupt_file() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    write_corrupt_tracking_file(&paths);

    let cache = empty_cache();
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let engine = InstallEngine::new(cache);

    let cleanup = registry.cleanup_removed_global_skills().unwrap();
    let sync = registry.sync_installed_global_skills(&engine).unwrap();

    assert_eq!(cleanup.removed, 0);
    assert_eq!(sync.refreshed, 0);
    assert!(sync.failures.is_empty());
    assert!(!paths.skill_tracking_file().exists());
    assert!(find_corrupt_tracking_file(&paths).exists());
}
