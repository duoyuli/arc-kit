use std::fs;
use std::process::{Command, Stdio};

fn apply_profile(agent: &str, name: &str, profile: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".arc-cli/providers");
    fs::create_dir_all(&providers_dir).unwrap();
    fs::write(providers_dir.join(format!("{agent}.toml")), profile).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_arc"))
        .args([
            "provider", "use", name, "--agent", agent, "--format", "json",
        ])
        .env("ARC_KIT_USER_HOME", temp.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stdout.contains("sk-xxx"));
    let result: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(result["ok"], true);
    temp
}

#[test]
fn global_infra_profile_generates_expected_codex_defaults() {
    let temp = apply_profile(
        "codex",
        "global-infra",
        r#"
[global-infra]
display_name = "Global Infra"
description = "订阅制"
base_url = "https://global-infra.net"
api_key = "sk-xxx"
"#,
    );
    let settings: toml::Value =
        toml::from_str(&fs::read_to_string(temp.path().join(".codex/config.toml")).unwrap())
            .unwrap();
    let expected: toml::Value = toml::from_str(
        r#"
model_provider = "OpenAI"

[model_providers.OpenAI]
name = "OpenAI"
base_url = "https://global-infra.net"
experimental_bearer_token = "sk-xxx"
wire_api = "responses"
requires_openai_auth = false
http_headers = { "x-openai-actor-authorization" = "local-image-extension" }
"#,
    )
    .unwrap();
    assert_eq!(settings, expected);
}

#[test]
fn deepseek_profile_generates_expected_claude_env() {
    let temp = apply_profile(
        "claude",
        "deepseek",
        r#"
[deepseek]
display_name = "DeepSeek"
description = "DeepSeek API 按量付费"
base_url = "https://api.deepseek.com/anthropic"
api_key = "sk-xxx"
ANTHROPIC_MODEL = "deepseek-flash[1m]"
ANTHROPIC_DEFAULT_OPUS_MODEL = "deepseek-flash[1m]"
ANTHROPIC_DEFAULT_SONNET_MODEL = "deepseek-flash[1m]"
ANTHROPIC_DEFAULT_HAIKU_MODEL = "deepseek-flash"
CLAUDE_CODE_SUBAGENT_MODEL = "deepseek-flash"
CLAUDE_CODE_EFFORT_LEVEL = "max"
CLAUDE_CODE_AUTO_COMPACT_WINDOW = 786432
"#,
    );
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(temp.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        settings,
        serde_json::json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "sk-xxx",
                "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
                "ANTHROPIC_MODEL": "deepseek-flash[1m]",
                "ANTHROPIC_DEFAULT_OPUS_MODEL": "deepseek-flash[1m]",
                "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-flash[1m]",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": "deepseek-flash",
                "CLAUDE_CODE_SUBAGENT_MODEL": "deepseek-flash",
                "CLAUDE_CODE_EFFORT_LEVEL": "max",
                "CLAUDE_CODE_AUTO_COMPACT_WINDOW": 786432
            }
        })
    );
}

#[test]
fn incomplete_provider_exits_one_without_switching_or_exposing_credentials() {
    for agent in ["claude", "codex"] {
        let temp = tempfile::tempdir().unwrap();
        let providers_dir = temp.path().join(".arc-cli/providers");
        fs::create_dir_all(&providers_dir).unwrap();
        fs::write(
            providers_dir.join(format!("{agent}.toml")),
            "[broken]\ndisplay_name = \"Broken\"\nbase_url = \"https://example.com\"\napi_key = \"sk-private-test\"\n",
        ).unwrap();
        let native_dir = temp.path().join(format!(".{agent}"));
        fs::create_dir_all(&native_dir).unwrap();
        fs::write(native_dir.join("auth.json"), "original auth").unwrap();

        for args in [
            vec!["provider", "list", "--format", "json"],
            vec![
                "provider", "use", "broken", "--agent", agent, "--format", "json",
            ],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_arc"))
                .args(args)
                .env("ARC_KIT_USER_HOME", temp.path())
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("description"), "{stderr}");
            assert!(!stderr.contains("sk-private-test"));
            assert!(!String::from_utf8_lossy(&output.stdout).contains("sk-private-test"));
            assert_eq!(
                fs::read_to_string(native_dir.join("auth.json")).unwrap(),
                "original auth"
            );
            assert!(!providers_dir.join("active.toml").exists());
            assert!(!native_dir.join("settings.json").exists());
            assert!(!native_dir.join("config.toml").exists());
        }
    }
}
