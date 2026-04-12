use std::collections::BTreeMap;
use std::path::PathBuf;

use arc_core::capability::{SubagentDefinition, validate_subagent_targets};
use arc_core::detect::{AgentInfo, DetectCache};

fn cache_with_agents(agents: &[&str]) -> DetectCache {
    let detected = agents
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                AgentInfo {
                    name: (*name).to_string(),
                    detected: true,
                    root: Some(PathBuf::from(format!("/tmp/{name}"))),
                    executable: Some(format!("/usr/bin/{name}")),
                    version: Some("test".to_string()),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    DetectCache::from_map(detected)
}

#[test]
fn codex_subagent_requires_description() {
    let cache = cache_with_agents(&["codex"]);
    let definition = SubagentDefinition {
        name: "reviewer".to_string(),
        description: None,
        targets: Some(vec!["codex".to_string()]),
        prompt_file: "reviewer.md".to_string(),
    };

    let err = validate_subagent_targets(&cache, &definition).unwrap_err();
    assert!(err.message.contains("description_required"));
}

#[test]
fn non_codex_subagent_allows_missing_description() {
    let cache = cache_with_agents(&["claude"]);
    let definition = SubagentDefinition {
        name: "reviewer".to_string(),
        description: None,
        targets: Some(vec!["claude".to_string()]),
        prompt_file: "reviewer.md".to_string(),
    };

    let targets = validate_subagent_targets(&cache, &definition).unwrap();
    assert_eq!(targets, vec!["claude".to_string()]);
}
