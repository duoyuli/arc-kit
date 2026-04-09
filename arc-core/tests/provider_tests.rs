use std::collections::BTreeMap;
use std::fs;

use arc_core::detect::DetectCache;
use arc_core::paths::ArcPaths;
use arc_core::provider::{
    ClaudeProviderConfig, CodexProviderConfig, ProviderInfo, ProviderSettings, apply_provider,
    load_providers_for_agent, read_active_provider, seed_default_providers,
    supported_provider_agents, supports_provider_agent, write_active_provider,
};

#[test]
fn provider_switch_writes_claude_settings() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let provider = ProviderInfo {
        name: "proxy".to_string(),
        display_name: "Proxy".to_string(),
        description: String::new(),
        agent: "claude".to_string(),
        settings: ProviderSettings::Claude(ClaudeProviderConfig {
            env_vars: BTreeMap::from([(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://example.com".to_string(),
            )]),
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let settings_path = temp.path().join(".claude").join("settings.json");
    let content = fs::read_to_string(settings_path).unwrap();
    assert!(content.contains("ANTHROPIC_BASE_URL"));
}

#[test]
fn provider_switch_clears_old_claude_env_vars() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let providers_dir = paths.providers_dir();
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("claude.toml"),
        "[old]\ndisplay_name = \"Old\"\nANTHROPIC_BASE_URL = \"https://old.example.com\"\nCUSTOM_VAR = \"old-val\"\n\n[new]\ndisplay_name = \"New\"\nANTHROPIC_BASE_URL = \"https://new.example.com\"\n",
    )
    .unwrap();

    let old_provider = ProviderInfo {
        name: "old".to_string(),
        display_name: "Old".to_string(),
        description: String::new(),
        agent: "claude".to_string(),
        settings: ProviderSettings::Claude(ClaudeProviderConfig {
            env_vars: BTreeMap::from([
                (
                    "ANTHROPIC_BASE_URL".to_string(),
                    "https://old.example.com".to_string(),
                ),
                ("CUSTOM_VAR".to_string(), "old-val".to_string()),
            ]),
        }),
    };
    apply_provider(&paths, &old_provider).unwrap();
    write_active_provider(&providers_dir, "claude", "old").unwrap();

    let new_provider = ProviderInfo {
        name: "new".to_string(),
        display_name: "New".to_string(),
        description: String::new(),
        agent: "claude".to_string(),
        settings: ProviderSettings::Claude(ClaudeProviderConfig {
            env_vars: BTreeMap::from([(
                "ANTHROPIC_BASE_URL".to_string(),
                "https://new.example.com".to_string(),
            )]),
        }),
    };
    apply_provider(&paths, &new_provider).unwrap();

    let settings_path = temp.path().join(".claude").join("settings.json");
    let content = fs::read_to_string(settings_path).unwrap();
    assert!(content.contains("https://new.example.com"));
    assert!(
        !content.contains("CUSTOM_VAR"),
        "old env var should be cleared"
    );
}

#[test]
fn provider_switch_writes_codex_auth() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let provider = ProviderInfo {
        name: "openai".to_string(),
        display_name: "OpenAI".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            api_key: Some("sk-test".to_string()),
            ..Default::default()
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let auth_path = temp.path().join(".codex").join("auth.json");
    let content = fs::read_to_string(auth_path).unwrap();
    assert!(content.contains("OPENAI_API_KEY"));
}

#[test]
fn provider_switch_snapshots_codex_auth_for_auth_provider() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let providers_dir = paths.providers_dir();
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("codex.toml"),
        "[official]\ndisplay_name = \"OpenAI\"\ndescription = \"auth login\"\n\n[proxy]\ndisplay_name = \"Proxy\"\napi_key = \"sk-proxy\"\nbase_url = \"https://example.com\"\n",
    )
    .unwrap();
    write_active_provider(&providers_dir, "codex", "official").unwrap();

    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    let original_auth = "{\n  \"account_id\": \"acct_123\",\n  \"id_token\": \"id_tok\",\n  \"refresh_token\": \"ref_tok\"\n}\n";
    fs::write(codex_dir.join("auth.json"), original_auth).unwrap();

    let proxy = load_providers_for_agent(&providers_dir, "codex")
        .into_iter()
        .find(|provider| provider.name == "proxy")
        .unwrap();
    apply_provider(&paths, &proxy).unwrap();

    let providers_content = fs::read_to_string(providers_dir.join("codex.toml")).unwrap();
    assert!(providers_content.contains("auth_json"));
    assert!(providers_content.contains("account_id"));

    let auth_content = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    assert!(auth_content.contains("\"OPENAI_API_KEY\": \"sk-proxy\""));
}

#[test]
fn provider_switch_writes_codex_base_url() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let provider = ProviderInfo {
        name: "proxy".to_string(),
        display_name: "My Proxy".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            api_key: Some("sk-test".to_string()),
            base_url: Some("https://example.com/codex".to_string()),
            ..Default::default()
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let config_path = temp.path().join(".codex").join("config.toml");
    let content = fs::read_to_string(config_path).unwrap();
    assert!(content.contains("model_provider = \"proxy\""));
    assert!(content.contains("[model_providers.proxy]"));
    assert!(content.contains("name = \"My Proxy\""));
    assert!(content.contains("base_url = \"https://example.com/codex\""));
    assert!(!content.contains("wire_api"));
}

#[test]
fn provider_switch_clears_codex_model_provider_for_official() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("config.toml"),
        "model = \"gpt-5.4\"\nmodel_provider = \"proxy\"\n[model_providers.proxy]\nname = \"proxy\"\nbase_url = \"https://old.example.com\"\n",
    )
    .unwrap();

    let provider = ProviderInfo {
        name: "official".to_string(),
        display_name: "Official".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            auth_json: Some("{\n  \"id_token\": \"id_tok\"\n}\n".to_string()),
            ..Default::default()
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let content = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(content.contains("model = \"gpt-5.4\""));
    assert!(!content.contains("model_provider = "));
    assert!(content.contains("[model_providers.proxy]"));
}

#[test]
fn provider_switch_restores_codex_auth_snapshot_for_official() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        "{\n  \"OPENAI_API_KEY\": \"sk-proxy\",\n  \"refresh_token\": \"stale\"\n}\n",
    )
    .unwrap();

    let provider = ProviderInfo {
        name: "official".to_string(),
        display_name: "Official".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            auth_json: Some(
                "{\n  \"account_id\": \"acct_123\",\n  \"id_token\": \"id_tok\",\n  \"refresh_token\": \"ref_tok\"\n}\n".to_string(),
            ),
            ..Default::default()
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let content = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    assert_eq!(
        content,
        "{\n  \"account_id\": \"acct_123\",\n  \"id_token\": \"id_tok\",\n  \"refresh_token\": \"ref_tok\"\n}\n"
    );
}

#[test]
fn provider_switch_restores_codex_auth_snapshot_and_overrides_mismatched_api_key() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    let provider = ProviderInfo {
        name: "hybrid".to_string(),
        display_name: "Hybrid".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            api_key: Some("sk-new".to_string()),
            auth_json: Some(
                "{\n  \"OPENAI_API_KEY\": \"sk-old\",\n  \"refresh_token\": \"ref_tok\"\n}\n"
                    .to_string(),
            ),
            ..Default::default()
        }),
    };

    apply_provider(&paths, &provider).unwrap();
    let content = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    assert!(content.contains("\"OPENAI_API_KEY\": \"sk-new\""));
    assert!(content.contains("\"refresh_token\": \"ref_tok\""));
    assert!(!content.contains("\"OPENAI_API_KEY\": \"sk-old\""));
}

#[test]
fn provider_switch_rolls_back_codex_auth_when_config_write_fails() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("auth.json"),
        "{\n  \"OPENAI_API_KEY\": \"sk-old\"\n}\n",
    )
    .unwrap();
    fs::write(
        codex_dir.join("config.toml"),
        "model = \"gpt-5.4\"\nmodel_providers = \"broken\"\n",
    )
    .unwrap();

    let provider = ProviderInfo {
        name: "proxy".to_string(),
        display_name: "Proxy".to_string(),
        description: String::new(),
        agent: "codex".to_string(),
        settings: ProviderSettings::Codex(CodexProviderConfig {
            api_key: Some("sk-new".to_string()),
            base_url: Some("https://example.com".to_string()),
            ..Default::default()
        }),
    };

    let err = apply_provider(&paths, &provider).unwrap_err();
    assert!(err.message.contains("model_providers"));

    let auth = fs::read_to_string(codex_dir.join("auth.json")).unwrap();
    let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(auth.contains("\"OPENAI_API_KEY\": \"sk-old\""));
    assert!(config.contains("model_providers = \"broken\""));
}

#[test]
fn active_provider_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    write_active_provider(&providers_dir, "claude", "proxy").unwrap();
    assert_eq!(
        read_active_provider(&providers_dir, "claude").as_deref(),
        Some("proxy")
    );
}

#[test]
fn provider_registry_reports_supported_agents() {
    let agents = supported_provider_agents();
    assert!(agents.contains(&"claude"));
    assert!(agents.contains(&"codex"));
    assert!(supports_provider_agent("claude"));
    assert!(supports_provider_agent("codex"));
    assert!(!supports_provider_agent("openclaw"));
}

#[test]
fn load_providers_parses_structured_codex_settings() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("codex.toml"),
        "[proxy]\ndisplay_name = \"Proxy\"\ndescription = \"desc\"\napi_key = \"sk-test\"\nbase_url = \"https://example.com\"\n",
    )
    .unwrap();

    let providers = load_providers_for_agent(&providers_dir, "codex");
    assert_eq!(providers.len(), 1);
    let ProviderSettings::Codex(config) = &providers[0].settings else {
        panic!("expected codex settings");
    };
    assert_eq!(config.api_key.as_deref(), Some("sk-test"));
    assert_eq!(config.base_url.as_deref(), Some("https://example.com"));
}

#[test]
fn load_providers_parses_codex_auth_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("codex.toml"),
        "[official]\ndisplay_name = \"OpenAI\"\nauth_json = '{\"account_id\":\"acct_123\"}'\n",
    )
    .unwrap();

    let providers = load_providers_for_agent(&providers_dir, "codex");
    assert_eq!(providers.len(), 1);
    let ProviderSettings::Codex(config) = &providers[0].settings else {
        panic!("expected codex settings");
    };
    assert_eq!(
        config.auth_json.as_deref(),
        Some("{\"account_id\":\"acct_123\"}")
    );
}

#[test]
fn load_providers_parses_claude_env_vars() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli").join("providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("claude.toml"),
        "[proxy]\ndisplay_name = \"Proxy\"\nANTHROPIC_BASE_URL = \"https://example.com\"\nANTHROPIC_AUTH_TOKEN = \"sk-ant-xxx\"\n",
    )
    .unwrap();

    let providers = load_providers_for_agent(&providers_dir, "claude");
    assert_eq!(providers.len(), 1);
    let ProviderSettings::Claude(config) = &providers[0].settings else {
        panic!("expected claude settings");
    };
    assert_eq!(
        config
            .env_vars
            .get("ANTHROPIC_BASE_URL")
            .map(|s| s.as_str()),
        Some("https://example.com")
    );
    assert_eq!(
        config
            .env_vars
            .get("ANTHROPIC_AUTH_TOKEN")
            .map(|s| s.as_str()),
        Some("sk-ant-xxx")
    );
    assert!(!config.env_vars.contains_key("display_name"));
}

#[test]
fn seed_default_providers_creates_official_for_detected_agent() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(temp.path().join(".claude")).unwrap();

    let cache = DetectCache::new(&paths);
    seed_default_providers(&paths, &cache);

    let providers = load_providers_for_agent(&paths.providers_dir(), "claude");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].name, "official");
    assert_eq!(providers[0].display_name, "Anthropic");
}

#[test]
fn seed_default_providers_skips_existing_config() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(temp.path().join(".claude")).unwrap();
    let providers_dir = paths.providers_dir();
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(
        providers_dir.join("claude.toml"),
        "[custom]\ndisplay_name = \"Custom\"\n",
    )
    .unwrap();

    let cache = DetectCache::new(&paths);
    seed_default_providers(&paths, &cache);

    let providers = load_providers_for_agent(&providers_dir, "claude");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].name, "custom");
}

#[test]
fn seed_default_providers_only_seeds_supported_agents() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());

    let cache = DetectCache::new(&paths);
    seed_default_providers(&paths, &cache);

    assert!(
        !paths.providers_dir().join("openclaw.toml").exists(),
        "openclaw has no provider backend, should never be seeded"
    );
}
