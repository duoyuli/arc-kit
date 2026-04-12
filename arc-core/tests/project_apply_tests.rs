use std::collections::BTreeMap;
use std::fs;

use arc_core::detect::{AgentInfo, DetectCache};
use arc_core::paths::ArcPaths;
use arc_core::project::{ProjectSkillApplyStatus, execute_project_apply, prepare_project_apply};

fn cache_with_claude(home: &std::path::Path) -> DetectCache {
    let agents = BTreeMap::from([(
        "claude".to_string(),
        AgentInfo {
            name: "claude".to_string(),
            detected: true,
            root: Some(home.join(".claude")),
            executable: Some("/usr/bin/claude".to_string()),
            version: Some("test".to_string()),
        },
    )]);
    DetectCache::from_map(agents)
}

#[test]
fn project_apply_service_installs_missing_project_skill() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");

    fs::create_dir_all(home.join(".arc-cli/skills/demo-skill")).unwrap();
    fs::create_dir_all(&repo).unwrap();
    fs::write(home.join(".arc-cli/skills/demo-skill/SKILL.md"), "# demo\n").unwrap();
    fs::write(
        repo.join("arc.toml"),
        "[skills]\nrequire = [\"demo-skill\"]\n",
    )
    .unwrap();

    let paths = ArcPaths::with_user_home(&home);
    paths.ensure_arc_home().unwrap();
    let cache = cache_with_claude(&home);

    let plan = prepare_project_apply(&paths, &cache, &repo).unwrap();
    assert_eq!(
        plan.effective.missing_installable,
        vec!["demo-skill".to_string()]
    );

    let execution =
        execute_project_apply(&paths, &cache, &plan, &["claude".to_string()], false).unwrap();

    assert_eq!(execution.skill_results.len(), 1);
    match &execution.skill_results[0].status {
        ProjectSkillApplyStatus::Installed { agents } => {
            assert_eq!(agents, &vec!["claude".to_string()]);
        }
        other => panic!("unexpected status: {other:?}"),
    }

    let installed = repo.join(".claude/skills/demo-skill");
    assert!(installed.exists());
}
