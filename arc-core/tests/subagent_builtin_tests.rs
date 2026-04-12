use arc_core::paths::ArcPaths;
use arc_core::subagent_registry::{
    SubagentEntryOrigin, find_global_subagent, load_merged_subagent_catalog,
};

#[test]
fn merged_subagent_catalog_includes_embedded_oh_my_agent_definitions() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());

    let entries = load_merged_subagent_catalog(&paths).unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry.definition.name == "arc-backend"),
        "expected built-in arc-backend subagent"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.definition.name == "arc-qa"),
        "expected built-in arc-qa subagent"
    );
}

#[test]
fn user_subagent_overrides_builtin_definition_by_name() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    std::fs::create_dir_all(paths.subagents_dir()).unwrap();
    std::fs::write(
        paths.subagents_dir().join("arc-backend.toml"),
        "name = \"arc-backend\"\ndescription = \"custom\"\nprompt_file = \"ignored.md\"\n",
    )
    .unwrap();
    std::fs::write(
        paths.subagents_dir().join("arc-backend.md"),
        "Custom prompt\n",
    )
    .unwrap();

    let entry = find_global_subagent(&paths, "arc-backend")
        .unwrap()
        .expect("entry should exist");
    assert_eq!(entry.origin, SubagentEntryOrigin::User);
    assert_eq!(entry.definition.description.as_deref(), Some("custom"));
    assert_eq!(entry.prompt_body, "Custom prompt\n");
}
