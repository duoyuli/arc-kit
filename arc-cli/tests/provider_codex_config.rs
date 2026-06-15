use std::fs;
use std::process::Command;

fn arc_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arc"))
}

#[test]
fn provider_use_codex_proxy_writes_native_provider_name_as_openai() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("codex.toml"),
        "[proxy]\ndisplay_name = \"My Proxy\"\napi_key = \"sk-test\"\nbase_url = \"https://example.com/codex\"\n",
    )
    .unwrap();

    let output = arc_cmd()
        .args(["provider", "use", "proxy", "--agent", "codex"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "provider use failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config_path = temp.path().join(".codex").join("config.toml");
    let content = fs::read_to_string(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    let config = toml::from_str::<toml::Value>(&content)
        .unwrap_or_else(|err| panic!("invalid TOML in {}: {err}", config_path.display()));
    let proxy = model_provider_table(&config, "proxy");

    assert_eq!(
        config.get("model_provider").and_then(toml::Value::as_str),
        Some("proxy")
    );
    assert_eq!(
        proxy.get("name").and_then(toml::Value::as_str),
        Some("OpenAI")
    );
    assert_eq!(
        proxy.get("base_url").and_then(toml::Value::as_str),
        Some("https://example.com/codex")
    );
    assert!(!content.contains("name = \"My Proxy\""));

    let auth_path = temp.path().join(".codex").join("auth.json");
    let auth_content = fs::read_to_string(&auth_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", auth_path.display()));
    let auth = serde_json::from_str::<serde_json::Value>(&auth_content)
        .unwrap_or_else(|err| panic!("invalid JSON in {}: {err}", auth_path.display()));
    assert_eq!(
        auth.get("OPENAI_API_KEY")
            .and_then(serde_json::Value::as_str),
        Some("sk-test")
    );
}

fn model_provider_table<'a>(config: &'a toml::Value, name: &str) -> &'a toml::Table {
    config
        .get("model_providers")
        .and_then(|providers| providers.get(name))
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("missing model provider table '{name}'"))
}
