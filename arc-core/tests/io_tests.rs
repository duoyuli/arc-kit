use arc_core::io::{
    atomic_write_string, read_to_string_if_exists, write_json_pretty, write_toml_pretty,
};

#[test]
fn atomic_write_string_persists_content() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("output.txt");

    atomic_write_string(&path, "hello").unwrap();

    assert_eq!(std::fs::read_to_string(path).unwrap(), "hello");
}

#[test]
fn read_to_string_if_exists_returns_none_for_missing_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing.txt");

    assert_eq!(read_to_string_if_exists(&path).unwrap(), None);
}

#[test]
fn pretty_writers_add_trailing_newline() {
    let temp = tempfile::tempdir().unwrap();
    let json_path = temp.path().join("config.json");
    let toml_path = temp.path().join("config.toml");

    write_json_pretty(&json_path, &serde_json::json!({"ok": true})).unwrap();
    write_toml_pretty(
        &toml_path,
        &toml::Value::Table(
            [("name".to_string(), toml::Value::String("demo".to_string()))]
                .into_iter()
                .collect(),
        ),
    )
    .unwrap();

    assert!(std::fs::read_to_string(json_path).unwrap().ends_with('\n'));
    assert!(std::fs::read_to_string(toml_path).unwrap().ends_with('\n'));
}
