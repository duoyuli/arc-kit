use std::fs;
use std::process::Command;

fn arc_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arc"))
}

#[test]
fn provider_use_codex_api_key_config_preserves_auth_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("codex.toml"),
        "[official]\ndisplay_name = \"OpenAI\"\ndescription = \"Subscription login\"\n\n[proxy]\ndisplay_name = \"My Proxy\"\ndescription = \"API access\"\napi_key = \"sk-test\"\nbase_url = \"https://global-infra.net\"\n",
    )
    .unwrap();
    fs::write(
        providers_dir.join("active.toml"),
        "[codex]\nactive = \"official\"\n",
    )
    .unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    let original_auth =
        "{\n  \"account_id\": \"acct-test\",\n  \"refresh_token\": \"refresh-test\"\n}\n";
    fs::write(codex_dir.join("auth.json"), original_auth).unwrap();

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
    let proxy = model_provider_table(&config, "OpenAI");

    assert_eq!(
        config.get("model_provider").and_then(toml::Value::as_str),
        Some("OpenAI")
    );
    assert_eq!(
        proxy.get("name").and_then(toml::Value::as_str),
        Some("OpenAI")
    );
    assert_eq!(
        proxy.get("base_url").and_then(toml::Value::as_str),
        Some("https://global-infra.net")
    );
    assert_eq!(
        proxy.get("wire_api").and_then(toml::Value::as_str),
        Some("responses")
    );
    assert_eq!(
        proxy
            .get("requires_openai_auth")
            .and_then(toml::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        proxy
            .get("experimental_bearer_token")
            .and_then(toml::Value::as_str),
        Some("sk-test")
    );
    let headers = proxy
        .get("http_headers")
        .and_then(toml::Value::as_table)
        .unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(
        headers["x-openai-actor-authorization"].as_str(),
        Some("local-image-extension")
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

    let output = arc_cmd()
        .args(["provider", "use", "official", "--agent", "codex"])
        .env("ARC_KIT_USER_HOME", temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "auth restore failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(auth_path).unwrap(), original_auth);
    let content = fs::read_to_string(config_path).unwrap();
    let config = toml::from_str::<toml::Value>(&content).unwrap();
    assert!(config.get("model_provider").is_none());
}

fn model_provider_table<'a>(config: &'a toml::Value, name: &str) -> &'a toml::Table {
    config
        .get("model_providers")
        .and_then(|providers| providers.get(name))
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("missing model provider table '{name}'"))
}
