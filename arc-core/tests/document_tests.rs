use arc_core::market::document::{MarketsDocument, read_markets_document, write_markets_document};
use arc_core::paths::ArcPaths;

#[test]
fn read_markets_document_returns_default_for_missing_file() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());

    let document = read_markets_document(&paths);
    assert_eq!(document.version, 3);
    assert!(document.resources.is_empty());
    assert!(document.sources.is_empty());
}

#[test]
fn write_and_read_markets_document_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let document = MarketsDocument {
        version: 3,
        updated_at: "123".to_string(),
        sources: [("source".to_string(), serde_json::json!({"id": "source"}))]
            .into_iter()
            .collect(),
        resources: vec![serde_json::json!({"id": "source/skill", "kind": "skill"})],
    };

    write_markets_document(&paths, &document).unwrap();
    let loaded = read_markets_document(&paths);

    assert_eq!(loaded.version, 3);
    assert_eq!(loaded.updated_at, "123");
    assert_eq!(loaded.sources.len(), 1);
    assert_eq!(loaded.resources.len(), 1);
}
