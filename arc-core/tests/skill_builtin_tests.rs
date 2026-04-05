use std::fs;

use arc_core::models::SkillOrigin;
use arc_core::skill::builtin;

#[test]
fn list_builtin_skills_returns_embedded_skills() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");

    let entries = builtin::list_builtin_skills(&cache_dir);
    assert!(
        !entries.is_empty(),
        "should have at least one built-in skill"
    );

    let entry = entries.iter().find(|e| e.name == "arc-cli-usage");
    assert!(
        entry.is_some(),
        "should contain the 'arc-cli-usage' seed skill"
    );

    let entry = entry.unwrap();
    assert_eq!(entry.origin, SkillOrigin::BuiltIn);
    assert!(!entry.summary.is_empty());
}

#[test]
fn materialize_extracts_builtin_skill_to_cache() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");

    let dest = builtin::materialize(&cache_dir, "arc-cli-usage").unwrap();
    assert!(dest.join("SKILL.md").is_file());
    let content = fs::read_to_string(dest.join("SKILL.md")).unwrap();
    assert!(content.contains("arc"));
}

#[test]
fn materialize_returns_error_for_unknown_skill() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("cache");
    let result = builtin::materialize(&cache_dir, "nonexistent");
    assert!(result.is_err());
}
