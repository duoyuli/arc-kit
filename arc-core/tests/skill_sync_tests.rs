use std::collections::BTreeMap;
use std::fs;

use arc_core::detect::{AgentInfo, DetectCache};
use arc_core::engine::InstallEngine;
use arc_core::models::ResourceKind;
use arc_core::paths::ArcPaths;
use arc_core::skill::tracking::track_global_skill_install;
use arc_core::skill::{SkillRegistry, run_global_skill_maintenance};

#[test]
fn cleanup_removes_tracked_global_install_when_skill_not_in_registry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.local_skills_dir()).unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(claude_root.join("skills")).unwrap();

    let orphan = claude_root.join("skills").join("gone-skill");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("SKILL.md"), "# gone\n").unwrap();

    let agents = BTreeMap::from([(
        "claude".to_string(),
        AgentInfo {
            name: "claude".to_string(),
            detected: true,
            root: Some(claude_root.clone()),
            executable: Some("/usr/bin/claude".to_string()),
            version: Some("test".to_string()),
        },
    )]);
    let cache = DetectCache::from_map(agents);
    let registry = SkillRegistry::new(paths.clone(), cache);
    track_global_skill_install(&claude_root.join("skills"), "claude", "gone-skill", &orphan)
        .unwrap();
    let report = registry.cleanup_removed_global_skills().unwrap();
    assert_eq!(report.removed, 1);
    assert!(!orphan.exists());
}

#[test]
fn cleanup_keeps_untracked_manual_install_when_skill_not_in_registry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.local_skills_dir()).unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(claude_root.join("skills")).unwrap();

    let manual = claude_root.join("skills").join("manual-skill");
    fs::create_dir_all(&manual).unwrap();
    fs::write(manual.join("SKILL.md"), "# manual\n").unwrap();

    let agents = BTreeMap::from([(
        "claude".to_string(),
        AgentInfo {
            name: "claude".to_string(),
            detected: true,
            root: Some(claude_root.clone()),
            executable: Some("/usr/bin/claude".to_string()),
            version: Some("test".to_string()),
        },
    )]);
    let cache = DetectCache::from_map(agents);
    let registry = SkillRegistry::new(paths.clone(), cache);
    let report = registry.cleanup_removed_global_skills().unwrap();
    assert_eq!(report.removed, 0);
    assert!(manual.exists());
}

#[test]
fn maintenance_bootstraps_legacy_arc_symlink_before_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let local_skill = paths.local_skills_dir().join("legacy-skill");
    fs::create_dir_all(&local_skill).unwrap();
    fs::write(local_skill.join("SKILL.md"), "# legacy\n").unwrap();

    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(claude_root.join("skills")).unwrap();
    let target = claude_root.join("skills").join("legacy-skill");
    std::os::unix::fs::symlink(&local_skill, &target).unwrap();
    fs::remove_dir_all(&local_skill).unwrap();

    let agents = BTreeMap::from([(
        "claude".to_string(),
        AgentInfo {
            name: "claude".to_string(),
            detected: true,
            root: Some(claude_root.clone()),
            executable: Some("/usr/bin/claude".to_string()),
            version: Some("test".to_string()),
        },
    )]);
    let cache = DetectCache::from_map(agents);

    let report = run_global_skill_maintenance(&paths, &cache).unwrap();
    assert_eq!(report.cleanup.removed, 1);
    assert!(target.symlink_metadata().is_err());
}

#[test]
fn sync_only_counts_actual_copy_changes() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.local_skills_dir()).unwrap();

    let local = paths.local_skills_dir().join("copy-skill");
    fs::create_dir_all(&local).unwrap();
    fs::write(local.join("SKILL.md"), "# before\n").unwrap();

    let openclaw_root = temp.path().join(".openclaw");
    fs::create_dir_all(openclaw_root.join("skills")).unwrap();

    let agents = BTreeMap::from([(
        "openclaw".to_string(),
        AgentInfo {
            name: "openclaw".to_string(),
            detected: true,
            root: Some(openclaw_root.clone()),
            executable: Some("/usr/bin/openclaw".to_string()),
            version: Some("test".to_string()),
        },
    )]);
    let cache = DetectCache::from_map(agents.clone());
    let registry = SkillRegistry::new(paths.clone(), cache.clone());
    let engine = InstallEngine::new(cache.clone());
    let skill = registry.find("copy-skill").expect("local skill");
    let src = registry.resolve_source_path(&skill).unwrap();
    engine
        .install_named(
            "copy-skill",
            &ResourceKind::Skill,
            &src,
            &["openclaw".to_string()],
        )
        .unwrap();
    track_global_skill_install(
        &openclaw_root.join("skills"),
        "openclaw",
        "copy-skill",
        &src,
    )
    .unwrap();

    let target = openclaw_root.join("skills").join("copy-skill");
    assert!(!target.is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "# before\n"
    );

    let report_unchanged = registry.sync_installed_global_skills(&engine).unwrap();
    assert_eq!(report_unchanged.refreshed, 0);

    fs::write(local.join("SKILL.md"), "# after\n").unwrap();

    let cache2 = DetectCache::from_map(agents.clone());
    let registry2 = SkillRegistry::new(paths.clone(), cache2.clone());
    let engine2 = InstallEngine::new(cache2);
    let report = registry2.sync_installed_global_skills(&engine2).unwrap();
    assert_eq!(report.refreshed, 1);
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "# after\n"
    );
}
