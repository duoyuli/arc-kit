use std::collections::BTreeMap;
use std::fs;

use arc_core::agent::{
    ProviderKind, SkillInstallStrategy, agent_spec, default_install_targets,
    ordered_agent_ids_for_resource_kind, project_skill_path, resource_install_subdir,
};
use arc_core::detect::{
    AgentInfo, DetectCache, detect_agent, detect_agents_for_install, project_skills_satisfied_all,
    project_skills_satisfied_any,
};
use arc_core::models::ResourceKind;
use arc_core::paths::ArcPaths;

#[test]
fn detect_agent_returns_info_for_known_agent() {
    let paths = ArcPaths::with_user_home("/tmp/arc-detect-test");
    let info = detect_agent(&paths, "codex").unwrap();
    assert_eq!(info.name, "codex");
    if info.detected {
        assert!(info.root.is_some());
        assert!(info.executable.is_some());
    } else {
        assert!(info.root.is_none());
    }
}

#[test]
fn detect_agent_rejects_unknown_agent() {
    let paths = ArcPaths::with_user_home("/tmp/arc-detect");
    let err = detect_agent(&paths, "unknown").unwrap_err();
    assert!(err.contains("unknown agent"));
}

#[test]
fn install_target_order_matches_supported_skill_agents() {
    let targets = ordered_agent_ids_for_resource_kind(&ResourceKind::Skill);
    assert!(targets.starts_with(&["claude".to_string(), "codex".to_string()]));
    assert_eq!(default_install_targets(&ResourceKind::Skill), targets);
}

#[test]
fn detect_agents_for_install_returns_subset_of_supported() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let targets = detect_agents_for_install(&paths, &ResourceKind::Skill);
    let all = ordered_agent_ids_for_resource_kind(&ResourceKind::Skill);
    assert!(!targets.is_empty());
    for t in &targets {
        assert!(all.contains(t), "unexpected agent: {t}");
    }
}

#[test]
fn openclaw_has_no_project_skill_layout() {
    let owl = agent_spec("openclaw").unwrap();
    assert!(!owl.supports_project_skills);
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    assert!(project_skill_path(&repo, "openclaw", "any-skill").is_none());
}

#[test]
fn coding_agent_spec_exposes_skill_subdir() {
    let cursor = agent_spec("cursor").unwrap();
    assert_eq!(cursor.skills_subdir, "skills-cursor");
}

#[test]
fn resource_install_subdir_matches_skill_spec_for_known_agents() {
    assert_eq!(
        resource_install_subdir(&ResourceKind::Skill, "cursor"),
        "skills-cursor"
    );
    assert_eq!(
        resource_install_subdir(&ResourceKind::Skill, "claude"),
        "skills"
    );
}

#[test]
fn resource_install_subdir_falls_back_to_kind_str_for_unknown_agent() {
    assert_eq!(
        resource_install_subdir(&ResourceKind::Skill, "unknown-agent"),
        "skill"
    );
}

#[test]
fn resource_install_subdir_non_skill_uses_as_str() {
    assert_eq!(
        resource_install_subdir(&ResourceKind::SubAgent, "claude"),
        "subagent"
    );
}

#[test]
fn coding_agent_spec_carries_install_strategy_and_provider_metadata() {
    let codex = agent_spec("codex").unwrap();
    assert_eq!(codex.skill_install_strategy, SkillInstallStrategy::Symlink);
    assert_eq!(codex.provider_kind, Some(ProviderKind::Codex));
    assert!(codex.provider_seed.is_some());

    let openclaw = agent_spec("openclaw").unwrap();
    assert_eq!(openclaw.skill_install_strategy, SkillInstallStrategy::Copy);
    assert_eq!(openclaw.provider_kind, None);
    assert!(openclaw.provider_seed.is_none());
}

#[test]
fn project_skills_any_vs_all_when_only_one_agent_has_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(repo.join(".claude/skills/demo")).unwrap();
    fs::write(repo.join(".claude/skills/demo/SKILL.md"), "# d\n").unwrap();

    let paths = ArcPaths::with_user_home(tmp.path());
    let mut agents = BTreeMap::new();
    agents.insert(
        "claude".to_string(),
        AgentInfo {
            name: "claude".to_string(),
            detected: true,
            root: Some(paths.user_home().join(".claude")),
            executable: Some("/a".to_string()),
            version: Some("1".to_string()),
        },
    );
    agents.insert(
        "codex".to_string(),
        AgentInfo {
            name: "codex".to_string(),
            detected: true,
            root: Some(paths.user_home().join(".codex")),
            executable: Some("/b".to_string()),
            version: Some("1".to_string()),
        },
    );
    let cache = DetectCache::from_map(agents);
    assert!(project_skills_satisfied_any(&cache, &repo, "demo"));
    assert!(!project_skills_satisfied_all(&cache, &repo, "demo"));
}

#[test]
fn codex_uses_project_skill_layout_under_repo_codex_dir() {
    let codex = agent_spec("codex").unwrap();
    assert!(codex.supports_project_skills);

    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    assert_eq!(
        project_skill_path(&repo, "codex", "demo-skill"),
        Some(repo.join("codex").join("skills").join("demo-skill"))
    );
}

#[test]
fn kimi_uses_home_and_project_skill_layout() {
    let kimi = agent_spec("kimi").unwrap();
    assert_eq!(kimi.skills_subdir, "skills");
    assert_eq!(kimi.executable, "kimi");
    assert_eq!(kimi.version_flag, "--version");
    assert!(kimi.supports_project_skills);
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    assert_eq!(
        project_skill_path(&repo, "kimi", "demo-skill"),
        Some(repo.join(".kimi").join("skills").join("demo-skill"))
    );
}
