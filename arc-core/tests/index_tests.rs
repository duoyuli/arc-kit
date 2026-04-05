use arc_core::market::index::MarketIndexStore;
use arc_core::paths::ArcPaths;

#[test]
fn refresh_from_manifest_file_url_writes_market_index_cache() {
    let temp = tempfile::tempdir().unwrap();
    let builtin_dir = temp.path().join("built-in");
    let market_dir = builtin_dir.join("market");
    std::fs::create_dir_all(&market_dir).unwrap();
    let manifest = builtin_dir.join("manifest.toml");
    let index = market_dir.join("index.toml");
    std::fs::write(
        &manifest,
        r#"
version = 1

[index.market]
path = "market/index.toml"
"#,
    )
    .unwrap();
    std::fs::write(
        &index,
        r#"
version = 1
updated_at = "2026-03-26"

[[repo]]
git_url = "https://github.com/example/builtin.git"
"#,
    )
    .unwrap();

    let paths = ArcPaths::with_user_home(temp.path().join("home"));
    let store = MarketIndexStore::new(paths.clone());
    let document = store
        .refresh_from_manifest_url(&format!("file://{}", manifest.display()))
        .unwrap();

    assert_eq!(document.repos.len(), 1);
    let cached = store.load_cached().unwrap();
    assert_eq!(
        cached.repos[0].git_url,
        "https://github.com/example/builtin.git"
    );
    assert!(paths.market_index_cache().exists());
}
