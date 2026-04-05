use std::collections::BTreeMap;

use arc_core::market::sources::{MarketSourcePatch, MarketSourceRegistry};
use arc_core::models::MarketSource;
use arc_core::paths::ArcPaths;

#[test]
fn generate_slug_uses_owner_and_repo_when_possible() {
    let registry = MarketSourceRegistry::new(ArcPaths::with_user_home("/tmp/arc-test"));
    assert_eq!(
        registry.generate_slug("https://github.com/openai/codex.git"),
        "openai-codex"
    );
}

#[test]
fn add_and_get_source_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let registry = MarketSourceRegistry::new(ArcPaths::with_user_home(temp.path()));

    let source = registry
        .add("https://github.com/openai/codex.git", "auto")
        .unwrap();

    let loaded = registry.get(&source.id).unwrap();
    assert_eq!(loaded.git_url, "https://github.com/openai/codex.git");
    assert_eq!(loaded.owner, "openai");
    assert_eq!(loaded.repo, "codex");
}

#[test]
fn update_source_persists_patch() {
    let temp = tempfile::tempdir().unwrap();
    let registry = MarketSourceRegistry::new(ArcPaths::with_user_home(temp.path()));
    let source = registry
        .add("https://github.com/openai/codex.git", "auto")
        .unwrap();

    let updated = registry
        .update_source(
            &source.id,
            MarketSourcePatch {
                status: Some("error".to_string()),
                resource_count: Some(7),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();

    assert_eq!(updated.status, "error");
    assert_eq!(updated.resource_count, 7);
}

#[test]
fn save_and_load_roundtrip_preserves_map() {
    let temp = tempfile::tempdir().unwrap();
    let registry = MarketSourceRegistry::new(ArcPaths::with_user_home(temp.path()));
    let sources = BTreeMap::from([(
        "community".to_string(),
        MarketSource {
            id: "community".to_string(),
            git_url: "https://github.com/example/community.git".to_string(),
            parser: "auto".to_string(),
            owner: "example".to_string(),
            repo: "community".to_string(),
            status: "ok".to_string(),
            last_updated_at: "123".to_string(),
            resource_count: 3,
        },
    )]);

    registry.save(&sources).unwrap();
    let loaded = registry.load();
    assert!(loaded.contains_key("community"));
    assert_eq!(loaded["community"].resource_count, 3);
}

#[test]
fn list_all_merges_cached_market_index_sources_with_local_sources() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    std::fs::create_dir_all(paths.markets_dir()).unwrap();
    std::fs::write(
        paths.market_index_cache(),
        r#"
version = 1
updated_at = "2026-03-26"

[[repo]]
git_url = "https://github.com/example/builtin.git"
"#,
    )
    .unwrap();
    let registry = MarketSourceRegistry::new(paths);
    registry
        .add("https://github.com/openai/codex.git", "auto")
        .unwrap();

    let sources = registry.list_all();
    assert_eq!(sources.len(), 2);
    let builtin = sources
        .iter()
        .find(|source| source.id == "example-builtin")
        .unwrap();
    assert_eq!(builtin.status, "indexed");
    assert_eq!(builtin.resource_count, 0);
}
