use std::fs;

use arc_core::paths::ArcPaths;
use arc_core::provider::{apply_provider, load_providers_for_agent};
use serde_json::json;

#[test]
fn claude_maps_common_credentials_and_preserves_extra_json_types() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.providers_dir()).unwrap();
    fs::write(
        paths.providers_dir().join("claude.toml"),
        r#"
[proxy]
display_name = "Team Proxy"
description = "Shared model endpoint"
base_url = "https://claude.example.com"
api_key = "sk-team"
ANTHROPIC_BASE_URL = "https://stale.example.com"
ANTHROPIC_AUTH_TOKEN = "sk-stale"
ANTHROPIC_DEFAULT_OPUS_MODEL = "team-opus"
API_TIMEOUT_MS = 60000
FEATURE_ENABLED = true
ALLOWED_MODELS = ["team-opus", "team-sonnet"]
CUSTOM_OPTIONS = { retries = 3, enabled = false }
"#,
    )
    .unwrap();
    let settings_path = temp.path().join(".claude/settings.json");
    fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    fs::write(
        &settings_path,
        r#"{"theme":"dark","env":{"UNRELATED":"keep"}}"#,
    )
    .unwrap();

    let providers = load_providers_for_agent(&paths.providers_dir(), "claude").unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].display_name, "Team Proxy");
    assert_eq!(providers[0].description, "Shared model endpoint");
    apply_provider(&paths, &providers[0]).unwrap();

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(settings_path).unwrap()).unwrap();
    assert_eq!(
        settings,
        json!({
            "theme": "dark",
            "env": {
                "UNRELATED": "keep",
                "ANTHROPIC_BASE_URL": "https://claude.example.com",
                "ANTHROPIC_AUTH_TOKEN": "sk-team",
                "ANTHROPIC_DEFAULT_OPUS_MODEL": "team-opus",
                "API_TIMEOUT_MS": 60000,
                "FEATURE_ENABLED": true,
                "ALLOWED_MODELS": ["team-opus", "team-sonnet"],
                "CUSTOM_OPTIONS": {"retries": 3, "enabled": false}
            }
        })
    );
}

#[test]
fn claude_auth_switch_removes_managed_credentials_and_extras() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.providers_dir()).unwrap();
    fs::write(
        paths.providers_dir().join("claude.toml"),
        r#"
[proxy]
display_name = "Proxy"
description = "API access"
base_url = "https://claude.example.com"
api_key = "sk-team"
CUSTOM_OPTIONS = { retries = 3 }

[official]
display_name = "Official"
description = "Subscription login"
base_url = ""
api_key = ""
FEATURE_ENABLED = true
"#,
    )
    .unwrap();
    let settings_path = temp.path().join(".claude/settings.json");
    fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    fs::write(&settings_path, r#"{"env":{"UNRELATED":"keep"}}"#).unwrap();

    let providers = load_providers_for_agent(&paths.providers_dir(), "claude").unwrap();
    let proxy = providers
        .iter()
        .find(|provider| provider.name == "proxy")
        .unwrap();
    let official = providers
        .iter()
        .find(|provider| provider.name == "official")
        .unwrap();
    apply_provider(&paths, proxy).unwrap();
    apply_provider(&paths, official).unwrap();

    let settings: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(settings_path).unwrap()).unwrap();
    assert_eq!(
        settings,
        json!({"env": {"UNRELATED": "keep", "FEATURE_ENABLED": true}})
    );
}

#[test]
fn provider_base_requires_metadata_and_paired_credentials_for_both_agents() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.providers_dir()).unwrap();
    let complete: toml::Table = toml::from_str(
        r#"
display_name = "Proxy"
description = "API access"
base_url = "https://example.com"
api_key = "sk-secret"
"#,
    )
    .unwrap();

    for agent in ["claude", "codex"] {
        let profile_path = paths.providers_dir().join(format!("{agent}.toml"));
        for field in ["display_name", "description", "base_url", "api_key"] {
            let mut invalid_values = vec![None, Some(toml::Value::Integer(123))];
            if matches!(field, "base_url" | "api_key") {
                invalid_values.push(Some(toml::Value::String(String::new())));
            }
            for invalid_value in invalid_values {
                let mut profile = complete.clone();
                match invalid_value {
                    None => {
                        profile.remove(field);
                    }
                    Some(value) => {
                        profile.insert(field.to_string(), value);
                    }
                }
                fs::write(
                    &profile_path,
                    format!("[proxy]\n{}", toml::to_string(&profile).unwrap()),
                )
                .unwrap();
                let err = load_providers_for_agent(&paths.providers_dir(), agent).unwrap_err();
                assert!(err.message.contains(field), "{agent}: {}", err.message);
                assert!(!err.message.contains("sk-secret"));
            }
        }

        for credentials in ["", "base_url = \"\"\napi_key = \"\"\n"] {
            fs::write(&profile_path, format!(
                "[official]\ndisplay_name = \"Official\"\ndescription = \"Subscription login\"\n{credentials}"
            )).unwrap();
            let providers = load_providers_for_agent(&paths.providers_dir(), agent).unwrap();
            assert_eq!(providers.len(), 1);
            apply_provider(&paths, &providers[0]).unwrap();
        }
    }
}

#[test]
fn codex_writes_extra_fields_inside_openai_and_removes_previous_extras_on_switch() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.providers_dir()).unwrap();
    fs::write(
        paths.providers_dir().join("codex.toml"),
        r#"
[custom]
display_name = "Team Proxy"
description = "Custom provider options"
base_url = "https://codex.example.com"
api_key = "sk-current"
name = "Custom Gateway"
request_max_retries = 7
supports_websockets = true
custom_routes = ["primary", "fallback"]
query_params = { region = "local" }
experimental_bearer_token = "sk-stale"

[custom.http_headers]
X-Team = "core"

[next]
display_name = "Next Proxy"
description = "Default options"
base_url = "https://next.example.com"
api_key = "sk-next"
"#,
    )
    .unwrap();
    let providers = load_providers_for_agent(&paths.providers_dir(), "codex").unwrap();
    let custom = providers
        .iter()
        .find(|provider| provider.name == "custom")
        .unwrap();
    let next = providers
        .iter()
        .find(|provider| provider.name == "next")
        .unwrap();
    apply_provider(&paths, custom).unwrap();

    let config_path = temp.path().join(".codex/config.toml");
    let config: toml::Value = toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(config["model_provider"].as_str(), Some("OpenAI"));
    let native = &config["model_providers"]["OpenAI"];
    assert_eq!(
        native["base_url"].as_str(),
        Some("https://codex.example.com")
    );
    assert_eq!(
        native["experimental_bearer_token"].as_str(),
        Some("sk-current")
    );
    assert_eq!(native["name"].as_str(), Some("OpenAI"));
    assert_eq!(native["request_max_retries"].as_integer(), Some(7));
    assert_eq!(native["supports_websockets"].as_bool(), Some(true));
    assert_eq!(native["custom_routes"].as_array().unwrap().len(), 2);
    assert_eq!(native["query_params"]["region"].as_str(), Some("local"));
    assert_eq!(native["http_headers"].as_table().unwrap().len(), 1);
    assert_eq!(native["http_headers"]["X-Team"].as_str(), Some("core"));
    for field in ["display_name", "description", "api_key"] {
        assert!(native.get(field).is_none());
    }
    for field in [
        "name",
        "request_max_retries",
        "supports_websockets",
        "custom_routes",
        "query_params",
        "http_headers",
    ] {
        assert!(config.get(field).is_none());
    }

    apply_provider(&paths, next).unwrap();
    let config: toml::Value = toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    let native = &config["model_providers"]["OpenAI"];
    assert_eq!(native["name"].as_str(), Some("OpenAI"));
    assert_eq!(
        native["experimental_bearer_token"].as_str(),
        Some("sk-next")
    );
    assert!(native.get("request_max_retries").is_none());
    assert!(native.get("custom_routes").is_none());
    assert!(native.get("query_params").is_none());
    assert_eq!(
        native["http_headers"]["x-openai-actor-authorization"].as_str(),
        Some("local-image-extension")
    );
}

#[test]
fn codex_keeps_headers_inline_across_existing_configs_and_provider_switches() {
    let temp = tempfile::tempdir().unwrap();
    let paths = ArcPaths::with_user_home(temp.path());
    fs::create_dir_all(paths.providers_dir()).unwrap();
    fs::write(
        paths.providers_dir().join("codex.toml"),
        r#"
[custom]
display_name = "Custom"
description = "Custom headers"
base_url = "https://custom.example.com"
api_key = "sk-custom"

[custom.http_headers]
"X.Dotted" = 'quoted "value" with \ backslash'
X-Note = "[model_providers.OpenAI.http_headers]"

[empty]
display_name = "Empty"
description = "Empty headers"
base_url = "https://empty.example.com"
api_key = "sk-empty"
http_headers = {}

[official]
display_name = "Official"
description = "Subscription login"
"#,
    )
    .unwrap();
    let config_path = temp.path().join(".codex/config.toml");
    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    fs::write(
        &config_path,
        r#"
model = "gpt-5.4"
model_provider = "OpenAI"

[model_providers.OpenAI]
name = "OpenAI"
base_url = "https://old.example.com"
experimental_bearer_token = "sk-old"

[model_providers.OpenAI.http_headers]
x-openai-actor-authorization = "old-actor"

[model_providers.other.http_headers]
X-Keep = "untouched"

[mcp_servers.demo]
command = "demo-server"
"#,
    )
    .unwrap();
    let providers = load_providers_for_agent(&paths.providers_dir(), "codex").unwrap();
    let custom = providers
        .iter()
        .find(|provider| provider.name == "custom")
        .unwrap();
    let official = providers
        .iter()
        .find(|provider| provider.name == "official")
        .unwrap();
    let empty = providers
        .iter()
        .find(|provider| provider.name == "empty")
        .unwrap();

    apply_provider(&paths, custom).unwrap();
    let first_output = fs::read_to_string(&config_path).unwrap();
    apply_provider(&paths, custom).unwrap();
    assert_eq!(fs::read_to_string(&config_path).unwrap(), first_output);

    for provider in [custom, official, empty] {
        apply_provider(&paths, provider).unwrap();
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(
            content
                .lines()
                .any(|line| { line.starts_with("http_headers = {") && line.ends_with('}') })
        );
        assert!(
            !content
                .lines()
                .any(|line| { line == "[model_providers.OpenAI.http_headers]" })
        );
        let config: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(config["model"].as_str(), Some("gpt-5.4"));
        assert_eq!(
            config["mcp_servers"]["demo"]["command"].as_str(),
            Some("demo-server")
        );
        assert_eq!(
            config["model_providers"]["other"]["http_headers"]["X-Keep"].as_str(),
            Some("untouched")
        );
        let headers = config["model_providers"]["OpenAI"]["http_headers"]
            .as_table()
            .unwrap();
        if provider.name == "empty" {
            assert!(headers.is_empty());
            assert!(content.contains("http_headers = {}"));
        } else {
            assert_eq!(headers.len(), 2);
            assert_eq!(
                headers["X.Dotted"].as_str(),
                Some("quoted \"value\" with \\ backslash")
            );
            assert_eq!(
                headers["X-Note"].as_str(),
                Some("[model_providers.OpenAI.http_headers]")
            );
        }
        if provider.name == "official" {
            assert!(config.get("model_provider").is_none());
        } else {
            assert_eq!(config["model_provider"].as_str(), Some("OpenAI"));
        }
    }
}
