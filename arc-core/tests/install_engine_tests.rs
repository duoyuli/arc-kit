use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use arc_core::detect::{AgentInfo, DetectCache};
use arc_core::engine::InstallEngine;
use arc_core::models::{ResourceInfo, ResourceKind};

fn install_engine_with_agents(home: &Path, agents: &[(&str, &str)]) -> InstallEngine {
    let detected = agents
        .iter()
        .map(|(name, root)| {
            (
                (*name).to_string(),
                AgentInfo {
                    name: (*name).to_string(),
                    detected: true,
                    root: Some(home.join(root)),
                    executable: Some(format!("/usr/bin/{name}")),
                    version: Some("test".to_string()),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    InstallEngine::new(DetectCache::from_map(detected))
}

#[test]
fn install_engine_installs_claude_skill_as_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(&claude_root).unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("claude", ".claude")]);
    let resource = ResourceInfo {
        id: "community/skill-a".to_string(),
        kind: ResourceKind::Skill,
        name: "skill-a".to_string(),
        source_id: "community".to_string(),
        summary: String::new(),
    };

    let source = temp.path().join("market").join("skill-a");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# skill").unwrap();

    let targets = vec!["claude".to_string()];
    let installed = engine.install(&resource, &source, &targets).unwrap();
    assert_eq!(installed, targets);

    let target = claude_root.join("skills").join("skill-a");
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn install_engine_installs_openclaw_skill_as_directory_copy() {
    let temp = tempfile::tempdir().unwrap();
    let openclaw_root = temp.path().join(".openclaw");
    fs::create_dir_all(&openclaw_root).unwrap();

    let source = temp.path().join("market").join("skill-b");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# skill").unwrap();
    fs::write(source.join("tool.txt"), "payload").unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("openclaw", ".openclaw")]);
    let resource = ResourceInfo {
        id: "community/skill-b".to_string(),
        kind: ResourceKind::Skill,
        name: "skill-b".to_string(),
        source_id: "community".to_string(),
        summary: String::new(),
    };

    engine
        .install(&resource, &source, &["openclaw".to_string()])
        .unwrap();

    let target = openclaw_root.join("skills").join("skill-b");
    assert!(target.is_dir());
    assert!(!target.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("tool.txt")).unwrap(),
        "payload"
    );
}

#[test]
fn install_named_symlinks_codex_skill() {
    let temp = tempfile::tempdir().unwrap();
    let codex_root = temp.path().join(".codex");
    fs::create_dir_all(&codex_root).unwrap();

    let source = temp.path().join("source").join("my-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# my skill").unwrap();
    fs::write(source.join("tool.txt"), "payload").unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("codex", ".codex")]);
    let installed = engine
        .install_named(
            "my-skill",
            &ResourceKind::Skill,
            &source,
            &["codex".to_string()],
        )
        .unwrap();
    assert_eq!(installed, vec!["codex".to_string()]);

    let target = codex_root.join("skills").join("my-skill");
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("tool.txt")).unwrap(),
        "payload"
    );
}

#[test]
fn uninstall_removes_installed_skill() {
    let temp = tempfile::tempdir().unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(&claude_root).unwrap();

    let source = temp.path().join("source").join("rm-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# rm").unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("claude", ".claude")]);
    engine
        .install_named(
            "rm-skill",
            &ResourceKind::Skill,
            &source,
            &["claude".to_string()],
        )
        .unwrap();

    let link = claude_root.join("skills").join("rm-skill");
    assert!(link.exists());

    let removed = engine
        .uninstall(
            "rm-skill",
            &ResourceKind::Skill,
            Some(&["claude".to_string()]),
        )
        .unwrap();
    assert!(removed);
    assert!(!link.exists());
}

#[test]
fn uninstall_returns_false_when_skill_not_on_disk() {
    let temp = tempfile::tempdir().unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(claude_root.join("skills")).unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("claude", ".claude")]);
    let removed = engine
        .uninstall(
            "nonexistent-skill",
            &ResourceKind::Skill,
            Some(&["claude".to_string()]),
        )
        .unwrap();
    assert!(!removed, "should return false when skill was not on disk");
}

#[test]
fn is_installed_for_detects_existing_skill() {
    let temp = tempfile::tempdir().unwrap();
    let claude_root = temp.path().join(".claude");
    fs::create_dir_all(&claude_root).unwrap();

    let source = temp.path().join("source").join("check-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# check").unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("claude", ".claude")]);
    assert!(!engine.is_installed_for("check-skill", &ResourceKind::Skill, "claude"));
    engine
        .install_named(
            "check-skill",
            &ResourceKind::Skill,
            &source,
            &["claude".to_string()],
        )
        .unwrap();
    assert!(engine.is_installed_for("check-skill", &ResourceKind::Skill, "claude"));
}

#[test]
fn install_named_project_writes_under_repo() {
    let temp = tempfile::tempdir().unwrap();
    let proj = temp.path().join("repo");
    fs::create_dir_all(&proj).unwrap();

    let engine = install_engine_with_agents(temp.path(), &[("claude", ".claude")]);
    let source = temp.path().join("source").join("proj-skill");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "# proj").unwrap();

    let installed = engine
        .install_named_project(
            "proj-skill",
            &ResourceKind::Skill,
            &source,
            &proj,
            &["claude".to_string()],
        )
        .unwrap();
    assert_eq!(installed, vec!["claude".to_string()]);

    let target = proj.join(".claude").join("skills").join("proj-skill");
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn resource_path_uses_per_agent_skill_subdir() {
    let temp = tempfile::tempdir().unwrap();
    let engine = install_engine_with_agents(temp.path(), &[("cursor", ".cursor")]);
    let root = temp.path().join(".cursor");
    let p = engine.resource_path(&root, &ResourceKind::Skill, "myskill", "cursor");
    assert!(p.ends_with("myskill"));
    assert!(
        p.to_string_lossy().contains("skills-cursor"),
        "expected skills-cursor in {}",
        p.display()
    );
}
