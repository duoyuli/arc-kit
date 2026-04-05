use std::fs;

use arc_core::models::SkillOrigin;
use arc_core::skill::local::scan_local_skills;

#[test]
fn scan_local_skills_finds_directories_with_skill_md() {
    let temp = tempfile::tempdir().unwrap();
    let skills_dir = temp.path();

    let skill_a = skills_dir.join("alpha");
    fs::create_dir_all(&skill_a).unwrap();
    fs::write(
        skill_a.join("SKILL.md"),
        "---\ndescription: Alpha skill\n---\nAlpha body",
    )
    .unwrap();

    let skill_b = skills_dir.join("beta");
    fs::create_dir_all(&skill_b).unwrap();
    fs::write(skill_b.join("SKILL.md"), "Beta first line").unwrap();

    let entries = scan_local_skills(skills_dir);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "alpha");
    assert_eq!(entries[0].summary, "Alpha skill");
    assert_eq!(entries[0].origin, SkillOrigin::Local);
    assert_eq!(entries[1].name, "beta");
    assert_eq!(entries[1].summary, "Beta first line");
}

#[test]
fn scan_local_skills_ignores_dirs_without_skill_md() {
    let temp = tempfile::tempdir().unwrap();
    let skills_dir = temp.path();

    let no_skill = skills_dir.join("no-skill");
    fs::create_dir_all(&no_skill).unwrap();
    fs::write(no_skill.join("README.md"), "not a skill").unwrap();

    let entries = scan_local_skills(skills_dir);
    assert!(entries.is_empty());
}

#[test]
fn scan_local_skills_returns_empty_for_nonexistent_dir() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("nonexistent");
    let entries = scan_local_skills(&missing);
    assert!(entries.is_empty());
}
