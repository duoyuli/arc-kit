use arc_core::market::catalog::CatalogManager;
use arc_core::models::{ResourceInfo, ResourceKind};
use arc_core::paths::ArcPaths;

fn sample_resource(id: &str, source_id: &str, name: &str) -> ResourceInfo {
    ResourceInfo {
        id: id.to_string(),
        kind: ResourceKind::Skill,
        name: name.to_string(),
        source_id: source_id.to_string(),
        summary: format!("summary for {name}"),
    }
}

#[test]
fn rebuild_and_get_resources_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let manager = CatalogManager::new(paths);
    let resources = vec![
        sample_resource("community/skill-a", "community", "skill-a"),
        sample_resource("other/skill-b", "other", "skill-b"),
    ];

    manager.rebuild(&resources).unwrap();

    let loaded: Vec<_> = manager
        .get_resources(Some(ResourceKind::Skill))
        .into_iter()
        .filter(|item| item.source_id == "community" || item.source_id == "other")
        .collect();
    assert_eq!(loaded.len(), 2);
    assert_eq!(
        manager.get_resource("community/skill-a").unwrap().name,
        "skill-a"
    );
}

#[test]
fn rebuild_source_replaces_single_source_only() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let manager = CatalogManager::new(paths);
    manager
        .rebuild(&[
            sample_resource("community/skill-a", "community", "skill-a"),
            sample_resource("other/skill-b", "other", "skill-b"),
        ])
        .unwrap();

    manager
        .rebuild_source(
            "community",
            &[sample_resource("community/skill-c", "community", "skill-c")],
        )
        .unwrap();

    let loaded: Vec<_> = manager
        .get_resources(Some(ResourceKind::Skill))
        .into_iter()
        .filter(|item| item.source_id == "community" || item.source_id == "other")
        .collect();
    assert_eq!(loaded.len(), 2);
    assert!(loaded.iter().any(|item| item.name == "skill-c"));
    assert!(loaded.iter().any(|item| item.name == "skill-b"));
    assert!(!loaded.iter().any(|item| item.name == "skill-a"));
}

#[test]
fn remove_source_resources_returns_removed_count() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let manager = CatalogManager::new(paths);
    manager
        .rebuild(&[
            sample_resource("community/skill-a", "community", "skill-a"),
            sample_resource("community/skill-b", "community", "skill-b"),
            sample_resource("other/skill-c", "other", "skill-c"),
        ])
        .unwrap();

    let removed = manager.remove_source_resources("community").unwrap();
    assert_eq!(removed, 2);
    let loaded: Vec<_> = manager
        .get_resources(Some(ResourceKind::Skill))
        .into_iter()
        .filter(|item| item.source_id == "community" || item.source_id == "other")
        .collect();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "skill-c");
}

#[test]
fn get_resources_returns_empty_without_local_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let manager = CatalogManager::new(paths);
    let loaded = manager.get_resources(Some(ResourceKind::Skill));

    assert!(loaded.is_empty());
}

#[test]
fn local_catalog_roundtrips_without_builtin_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let manager = CatalogManager::new(paths);
    manager
        .rebuild(&[ResourceInfo {
            id: "builtin/skill-a".to_string(),
            kind: ResourceKind::Skill,
            name: "skill-a".to_string(),
            source_id: "builtin".to_string(),
            summary: "from-local".to_string(),
        }])
        .unwrap();

    let resource = manager.get_resource("builtin/skill-a").unwrap();
    assert_eq!(resource.summary, "from-local");
}
