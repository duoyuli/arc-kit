use std::fs;

use arc_core::market::scanner::{find_skill_directory, scan_repo, scan_skills};

#[test]
fn scan_skills_reads_frontmatter_description() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("my-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: test summary\n---\n# Title\n",
    )
    .unwrap();

    let resources = scan_skills(temp.path(), Some("community"));
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id, "community/my-skill");
    assert_eq!(resources[0].summary, "test summary");
}

#[test]
fn scan_repo_toml_parser_matches_auto() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("x-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: x\n---\n# X\n",
    )
    .unwrap();

    let auto = scan_repo(temp.path(), "auto", Some("src"));
    let toml = scan_repo(temp.path(), "toml", Some("src"));
    assert_eq!(auto, toml);
    assert_eq!(auto.len(), 1);
}

#[test]
fn find_skill_directory_locates_named_skill() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("nested").join("skill-a");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# skill").unwrap();

    let found = find_skill_directory(temp.path(), "skill-a").unwrap();
    assert_eq!(found, skill_dir);
}
