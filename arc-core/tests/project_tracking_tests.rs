use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use arc_core::detect::{AgentInfo, DetectCache};
use arc_core::paths::ArcPaths;
use arc_core::project::skills::{ProjectSkillOptions, reconcile_project_skills};
use arc_core::project::{execute_project_apply_with_options, prepare_project_apply_with_mode};
use arc_core::skill::SkillRegistry;
use arc_core::skill::install::{
    InstallScope, InstallStatus, inspect_install_ledger, install_global_skill,
    uninstall_global_skill,
};

fn cache(home: &Path, names: &[&str]) -> DetectCache {
    DetectCache::from_map(
        names
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    AgentInfo {
                        name: name.to_string(),
                        detected: true,
                        root: Some(home.join(format!(".{name}"))),
                        executable: Some(format!("/fake/{name}")),
                        version: Some("test".to_string()),
                    },
                )
            })
            .collect(),
    )
}

fn setup() -> (tempfile::TempDir, ArcPaths, PathBuf, PathBuf, DetectCache) {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let paths = ArcPaths::with_user_home(&home);
    paths.ensure_arc_home().unwrap();
    fs::write(
        paths.market_index_cache(),
        "version = 1\nupdated_at = \"2026-09-14\"\n",
    )
    .unwrap();
    let source = paths.local_skills_dir().join("demo");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# demo\n").unwrap();
    let root = temp.path().join("project");
    fs::create_dir_all(&root).unwrap();
    write_requirements(&root, &["demo"]);
    let cache = cache(&home, &["codex", "claude"]);
    (temp, paths, root, source, cache)
}

fn write_requirements(root: &Path, names: &[&str]) {
    let body = format!(
        "[skills]\nrequire = {}\n",
        serde_json::to_string(names).unwrap()
    );
    fs::write(root.join("arc.toml"), body).unwrap();
}

fn apply(
    paths: &ArcPaths,
    cache: &DetectCache,
    root: &Path,
    options: ProjectSkillOptions,
) -> arc_core::skill::install::InstallReport {
    let plan = prepare_project_apply_with_mode(paths, cache, root, options.dry_run).unwrap();
    execute_project_apply_with_options(paths, cache, &plan, &options)
        .unwrap()
        .installs
}

#[test]
fn records_actual_paths_and_isolates_global_and_two_projects() {
    let (temp, paths, root, source, cache) = setup();
    let second = temp.path().join("other-project");
    fs::create_dir_all(&second).unwrap();
    write_requirements(&second, &["demo"]);
    let options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    assert!(install_global_skill(&paths, &cache, "demo", &source, &options.agents).ok());
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    assert!(apply(&paths, &cache, &second, options.clone()).ok());
    let ledger = inspect_install_ledger(&paths).unwrap();
    assert_eq!(ledger.schema_version, 2);
    assert_eq!(ledger.installs.len(), 3);
    assert_eq!(
        ledger
            .installs
            .iter()
            .filter(|record| record.scope == InstallScope::Project)
            .count(),
        2
    );
    for record in &ledger.installs {
        assert!(record.target_path.is_absolute());
        assert!(record.target_path.is_symlink());
        assert_eq!(
            fs::read_link(&record.target_path).unwrap(),
            record.source_path
        );
    }
    write_requirements(&root, &[]);
    let report = apply(
        &paths,
        &DetectCache::from_map(BTreeMap::new()),
        &root,
        ProjectSkillOptions::default(),
    );
    assert!(report.ok(), "{report:?}");
    assert_eq!(report.items[0].status, InstallStatus::Removed);
    assert!(!root.join(".codex/skills/demo").is_symlink());
    assert!(second.join(".codex/skills/demo").is_symlink());
    assert!(paths.user_home().join(".codex/skills/demo").is_symlink());
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 2);
    assert!(uninstall_global_skill(&paths, &cache, "demo", None).ok());
    assert!(second.join(".codex/skills/demo").is_symlink());
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 1);
}

#[test]
fn unmanaged_targets_require_explicit_adoption_and_conflicts_block_cleanup() {
    let (_temp, paths, root, source, cache) = setup();
    let target = root.join(".codex/skills/demo");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&source, &target).unwrap();
    let mut options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    let unmanaged = apply(&paths, &cache, &root, options.clone());
    assert!(!unmanaged.ok());
    assert_eq!(unmanaged.items[0].status, InstallStatus::Unmanaged);
    assert!(inspect_install_ledger(&paths).unwrap().installs.is_empty());
    options.adopt_existing = true;
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    fs::remove_file(&target).unwrap();
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("notes.txt"), "manual changes").unwrap();
    write_requirements(&root, &[]);
    let report = apply(&paths, &cache, &root, options);
    assert!(!report.ok());
    assert_eq!(report.items[0].status, InstallStatus::Conflict);
    assert_eq!(
        fs::read_to_string(target.join("notes.txt")).unwrap(),
        "manual changes"
    );
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 1);
}

#[test]
fn clean_handles_missing_manifest_and_dangling_owned_links_without_detection() {
    let (_temp, paths, root, source, cache) = setup();
    assert!(
        apply(
            &paths,
            &cache,
            &root,
            ProjectSkillOptions {
                agents: vec!["codex".to_string()],
                ..Default::default()
            }
        )
        .ok()
    );
    fs::remove_dir_all(source).unwrap();
    fs::remove_file(root.join("arc.toml")).unwrap();
    let report = reconcile_project_skills(
        &paths,
        &DetectCache::from_map(BTreeMap::new()),
        &root,
        &[],
        &ProjectSkillOptions::default(),
        true,
        &[],
    );
    assert!(report.ok(), "{report:?}");
    assert!(!root.join(".codex/skills/demo").is_symlink());
    assert!(inspect_install_ledger(&paths).unwrap().installs.is_empty());
    let registry = SkillRegistry::new(paths, cache);
    assert_eq!(registry.cleanup_removed_global_skills().unwrap().removed, 0);
}

#[test]
fn cleanup_only_agent_filter_and_market_maintenance_preserve_other_scopes() {
    let (_temp, paths, root, source, cache) = setup();
    assert!(
        apply(
            &paths,
            &cache,
            &root,
            ProjectSkillOptions {
                all_agents: true,
                ..Default::default()
            }
        )
        .ok()
    );
    assert!(install_global_skill(&paths, &cache, "demo", &source, &["codex".to_string()]).ok());
    fs::remove_dir_all(&source).unwrap();
    let registry = SkillRegistry::new(paths.clone(), DetectCache::from_map(BTreeMap::new()));
    assert_eq!(registry.cleanup_removed_global_skills().unwrap().removed, 1);
    assert!(root.join(".codex/skills/demo").is_symlink());
    assert!(root.join(".claude/skills/demo").is_symlink());
    write_requirements(&root, &[]);
    let report = apply(
        &paths,
        &cache,
        &root,
        ProjectSkillOptions {
            agents: vec!["codex".to_string()],
            ..Default::default()
        },
    );
    assert!(report.ok(), "{report:?}");
    assert!(!root.join(".codex/skills/demo").is_symlink());
    assert!(root.join(".claude/skills/demo").is_symlink());
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 1);
}

#[test]
fn unavailable_required_source_blocks_new_cleanup_without_forgetting_records() {
    let (_temp, paths, root, source, cache) = setup();
    let second = paths.local_skills_dir().join("second");
    fs::create_dir_all(&second).unwrap();
    fs::write(second.join("SKILL.md"), "# second").unwrap();
    write_requirements(&root, &["demo", "second"]);
    let options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    let before = fs::read(paths.skill_tracking_file()).unwrap();
    fs::remove_dir_all(source).unwrap();
    write_requirements(&root, &["demo"]);
    let report = apply(&paths, &cache, &root, options);
    assert!(!report.ok());
    assert!(
        report
            .items
            .iter()
            .any(|item| item.skill == "demo" && item.status == InstallStatus::Unavailable)
    );
    assert!(
        report
            .items
            .iter()
            .any(|item| item.skill == "second" && item.status == InstallStatus::NotExecuted)
    );
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), before);
    assert!(root.join(".codex/skills/second").is_symlink());
}

#[test]
fn dry_run_is_read_only_and_reports_refresh_without_retargeting() {
    let (_temp, paths, root, source, cache) = setup();
    let mut options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    options.dry_run = true;
    let preview = apply(&paths, &cache, &root, options.clone());
    assert!(preview.ok());
    assert!(!paths.skill_tracking_file().exists());
    assert!(!paths.state_dir().join("skills/installs.lock").exists());
    assert!(!root.join(".codex").exists());
    options.dry_run = false;
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    let ledger = fs::read(paths.skill_tracking_file()).unwrap();
    let target = root.join(".codex/skills/demo");
    let original = fs::read_link(&target).unwrap();
    let report = apply(
        &paths,
        &cache,
        &root,
        ProjectSkillOptions {
            dry_run: true,
            ..options
        },
    );
    assert!(report.ok());
    assert!(!report.has_changes());
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), ledger);
    assert_eq!(fs::read_link(target).unwrap(), original);
    assert!(source.exists());
}

#[test]
fn ancestor_escape_and_shared_physical_targets_are_preflight_conflicts() {
    let (temp, paths, root, _source, cache) = setup();
    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(root.join(".codex")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join(".codex/skills")).unwrap();
    let report = apply(
        &paths,
        &cache,
        &root,
        ProjectSkillOptions {
            agents: vec!["codex".to_string()],
            ..Default::default()
        },
    );
    assert!(!report.ok());
    assert!(!outside.join("demo").exists());
    fs::remove_file(root.join(".codex/skills")).unwrap();
    fs::create_dir_all(root.join(".codex/skills")).unwrap();
    fs::create_dir_all(root.join(".claude")).unwrap();
    std::os::unix::fs::symlink(root.join(".codex/skills"), root.join(".claude/skills")).unwrap();
    let collision = apply(
        &paths,
        &cache,
        &root,
        ProjectSkillOptions {
            all_agents: true,
            ..Default::default()
        },
    );
    assert!(!collision.ok());
    assert!(
        collision
            .items
            .iter()
            .any(|item| item.status == InstallStatus::Conflict)
    );
    assert!(!root.join(".codex/skills/demo").is_symlink());
    assert!(inspect_install_ledger(&paths).unwrap().installs.is_empty());
}

#[test]
fn historical_project_layout_is_retired_after_current_destination_is_installed() {
    let (_temp, paths, root, _source, _) = setup();
    let cache = cache(paths.user_home(), &["cursor"]);
    let options = ProjectSkillOptions {
        agents: vec!["cursor".to_string()],
        ..Default::default()
    };
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    let old = root.join(".cursor/skills-cursor/demo");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::rename(root.join(".cursor/skills/demo"), &old).unwrap();
    let mut body: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.skill_tracking_file()).unwrap()).unwrap();
    body["installs"][0]["target_path"] = serde_json::to_value(
        fs::canonicalize(old.parent().unwrap())
            .unwrap()
            .join("demo"),
    )
    .unwrap();
    fs::write(
        paths.skill_tracking_file(),
        serde_json::to_vec_pretty(&body).unwrap(),
    )
    .unwrap();
    let report = apply(&paths, &cache, &root, options);
    assert!(report.ok(), "{report:?}");
    assert_eq!(report.items[0].status, InstallStatus::Installed);
    assert_eq!(report.items[1].status, InstallStatus::Removed);
    assert!(!old.is_symlink());
    assert!(root.join(".cursor/skills/demo").is_symlink());
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 1);
}

#[test]
fn partial_global_install_keeps_records_for_completed_targets() {
    let (_temp, paths, _root, source, cache) = setup();
    let report = install_global_skill(
        &paths,
        &cache,
        "demo",
        &source,
        &["codex".to_string(), "unknown-agent".to_string()],
    );
    assert!(!report.ok());
    assert_eq!(report.items[0].status, InstallStatus::Installed);
    assert_eq!(report.items[1].status, InstallStatus::Failed);
    let state = inspect_install_ledger(&paths).unwrap();
    assert_eq!(state.installs.len(), 1);
    assert!(state.installs[0].target_path.is_symlink());
}

#[test]
fn source_retarget_refreshes_owned_links_but_content_changes_keep_the_link() {
    use std::os::unix::fs::MetadataExt;
    let (temp, paths, root, source, cache) = setup();
    let old = temp.path().join("old-source");
    fs::create_dir(&old).unwrap();
    fs::write(old.join("SKILL.md"), "old").unwrap();
    let engine = arc_core::engine::InstallEngine::with_paths(paths.clone(), cache.clone());
    engine
        .install_named_project(
            "demo",
            &arc_core::models::ResourceKind::Skill,
            &old,
            &root,
            &["codex".to_string()],
        )
        .unwrap();
    let refreshed = apply(&paths, &cache, &root, ProjectSkillOptions::default());
    assert!(refreshed.ok(), "{refreshed:?}");
    assert_eq!(refreshed.items[0].status, InstallStatus::Refreshed);
    let target = root.join(".codex/skills/demo");
    assert_eq!(fs::read_link(&target).unwrap(), source);
    let inode = fs::symlink_metadata(&target).unwrap().ino();
    fs::write(source.join("SKILL.md"), "new contents").unwrap();
    let unchanged = apply(&paths, &cache, &root, ProjectSkillOptions::default());
    assert!(unchanged.ok());
    assert!(!unchanged.has_changes());
    assert_eq!(fs::symlink_metadata(&target).unwrap().ino(), inode);
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "new contents"
    );
    assert_eq!(fs::read_to_string(old.join("SKILL.md")).unwrap(), "old");
}

#[test]
fn a_moved_project_requires_adoption_and_preserves_the_old_root_record() {
    let (temp, paths, root, _source, cache) = setup();
    assert!(
        apply(
            &paths,
            &cache,
            &root,
            ProjectSkillOptions {
                agents: vec!["codex".to_string()],
                ..Default::default()
            }
        )
        .ok()
    );
    let moved = temp.path().join("moved-project");
    fs::rename(&root, &moved).unwrap();
    let options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    let untracked = apply(&paths, &cache, &moved, options.clone());
    assert!(!untracked.ok());
    assert_eq!(untracked.items[0].status, InstallStatus::Unmanaged);
    assert!(
        apply(
            &paths,
            &cache,
            &moved,
            ProjectSkillOptions {
                adopt_existing: true,
                ..options
            }
        )
        .ok()
    );
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 2);
    let before = fs::read(paths.skill_tracking_file()).unwrap();
    let missing = reconcile_project_skills(
        &paths,
        &cache,
        &root,
        &[],
        &ProjectSkillOptions::default(),
        true,
        &[],
    );
    assert!(!missing.ok());
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), before);
    assert!(moved.join(".codex/skills/demo").is_symlink());
}

#[test]
fn all_agents_combines_recorded_agents_with_new_detected_agents() {
    let (_temp, paths, root, _source, original) = setup();
    assert!(
        apply(
            &paths,
            &original,
            &root,
            ProjectSkillOptions {
                agents: vec!["codex".to_string()],
                ..Default::default()
            }
        )
        .ok()
    );
    let only_claude = cache(paths.user_home(), &["claude"]);
    let report = apply(
        &paths,
        &only_claude,
        &root,
        ProjectSkillOptions {
            all_agents: true,
            ..Default::default()
        },
    );
    assert!(report.ok(), "{report:?}");
    assert!(
        report
            .items
            .iter()
            .any(|item| item.agent == "codex" && item.status == InstallStatus::Kept)
    );
    assert!(
        report
            .items
            .iter()
            .any(|item| item.agent == "claude" && item.status == InstallStatus::Installed)
    );
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 2);
}

#[test]
fn one_clean_conflict_blocks_all_new_removals() {
    let (_temp, paths, root, _source, cache) = setup();
    assert!(
        apply(
            &paths,
            &cache,
            &root,
            ProjectSkillOptions {
                all_agents: true,
                ..Default::default()
            }
        )
        .ok()
    );
    let modified = root.join(".codex/skills/demo");
    fs::remove_file(&modified).unwrap();
    fs::create_dir(&modified).unwrap();
    fs::write(modified.join("notes.txt"), "manual").unwrap();
    let before = fs::read(paths.skill_tracking_file()).unwrap();
    let report = reconcile_project_skills(
        &paths,
        &cache,
        &root,
        &[],
        &ProjectSkillOptions {
            all_agents: true,
            ..Default::default()
        },
        true,
        &[],
    );
    assert!(!report.ok());
    assert!(
        report
            .items
            .iter()
            .any(|item| item.agent == "claude" && item.status == InstallStatus::NotExecuted)
    );
    assert!(root.join(".claude/skills/demo").is_symlink());
    assert_eq!(
        fs::read_to_string(modified.join("notes.txt")).unwrap(),
        "manual"
    );
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), before);
}

#[test]
fn modified_or_inaccessible_copies_are_preserved_during_refresh() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, paths, _root, source, _) = setup();
    let cache = cache(paths.user_home(), &["openclaw"]);
    assert!(install_global_skill(&paths, &cache, "demo", &source, &["openclaw".to_string()]).ok());
    let target = paths.user_home().join(".openclaw/skills/demo");
    let before = fs::read(paths.skill_tracking_file()).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).unwrap();
    let inaccessible =
        install_global_skill(&paths, &cache, "demo", &source, &["openclaw".to_string()]);
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!inaccessible.ok());
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), before);
    fs::write(target.join("SKILL.md"), "user edit").unwrap();
    fs::write(source.join("SKILL.md"), "source update").unwrap();
    let modified = install_global_skill(&paths, &cache, "demo", &source, &["openclaw".to_string()]);
    assert!(!modified.ok());
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "user edit"
    );
    assert_eq!(fs::read(paths.skill_tracking_file()).unwrap(), before);
}

#[test]
fn copy_ownership_distinguishes_file_boundaries_before_removal() {
    let (_temp, paths, _root, source, _) = setup();
    let detected = cache(paths.user_home(), &["openclaw"]);
    fs::write(source.join("a"), "bfileX").unwrap();
    assert!(
        install_global_skill(
            &paths,
            &detected,
            "demo",
            &source,
            &["openclaw".to_string()]
        )
        .ok()
    );
    let target = paths.user_home().join(".openclaw/skills/demo");
    // 旧算法把 a 的内容与新增文件 b 的名称、类型、内容拼成同一串字节。
    fs::write(target.join("a"), "").unwrap();
    fs::write(target.join("b"), "X").unwrap();
    let report = uninstall_global_skill(&paths, &detected, "demo", None);
    assert!(!report.ok(), "{report:?}");
    assert_eq!(report.items[0].status, InstallStatus::Conflict);
    assert_eq!(fs::read_to_string(target.join("b")).unwrap(), "X");
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 1);
}

#[test]
fn nested_manifests_and_git_worktrees_have_independent_install_roots() {
    let (temp, paths, root, _source, cache) = setup();
    let run_git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(&root)
            .args([
                "-c",
                "user.name=Arc Tests",
                "-c",
                "user.email=arc-tests@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init", "-q", "-b", "main"]);
    run_git(&["add", "arc.toml"]);
    run_git(&["commit", "-q", "-m", "fixture"]);
    let worktree = temp.path().join("worktree");
    run_git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "other",
        worktree.to_str().unwrap(),
    ]);
    let nested = root.join("packages/service");
    let child = nested.join("src/deep");
    fs::create_dir_all(&child).unwrap();
    write_requirements(&nested, &["demo"]);
    let options = ProjectSkillOptions {
        agents: vec!["codex".to_string()],
        ..Default::default()
    };
    assert!(apply(&paths, &cache, &child, options.clone()).ok());
    assert!(nested.join(".codex/skills/demo").is_symlink());
    assert!(!root.join(".codex/skills/demo").exists());
    assert!(apply(&paths, &cache, &root, options.clone()).ok());
    assert!(apply(&paths, &cache, &worktree, options).ok());
    assert_eq!(inspect_install_ledger(&paths).unwrap().installs.len(), 3);
    write_requirements(&root, &[]);
    assert!(apply(&paths, &cache, &root, ProjectSkillOptions::default()).ok());
    assert!(nested.join(".codex/skills/demo").is_symlink());
    assert!(worktree.join(".codex/skills/demo").is_symlink());
}
