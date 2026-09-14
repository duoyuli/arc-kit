use log::info;
use serde_json::Value;

use crate::error::{ArcError, Result};
use crate::io::{read_json_map, write_json_pretty};
use crate::paths::ArcPaths;

use super::{ClaudeProviderConfig, ProviderBaseConfig, ProviderInfo, ProviderSettings};

pub fn parse_provider_config(
    base: &ProviderBaseConfig,
    extra: &toml::Table,
) -> Result<ProviderSettings> {
    let mut env_vars = extra
        .iter()
        .map(|(key, value)| {
            serde_json::to_value(value)
                .map(|value| (key.clone(), value))
                .map_err(|_| {
                    ArcError::new(format!("failed to convert Claude field '{key}' to JSON"))
                })
        })
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    if let Some(base_url) = &base.base_url {
        env_vars.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            Value::String(base_url.clone()),
        );
    }
    if let Some(api_key) = &base.api_key {
        env_vars.insert(
            "ANTHROPIC_AUTH_TOKEN".to_string(),
            Value::String(api_key.clone()),
        );
    }
    Ok(ProviderSettings::Claude(ClaudeProviderConfig { env_vars }))
}

pub fn apply_provider(
    paths: &ArcPaths,
    old: Option<&ProviderInfo>,
    new: &ProviderInfo,
) -> Result<()> {
    let ProviderSettings::Claude(new_config) = &new.settings else {
        return Err(ArcError::new(format!(
            "provider '{}' does not have Claude settings",
            new.name
        )));
    };

    let settings_path = paths.user_home().join(".claude").join("settings.json");
    let mut settings = read_json_map(&settings_path);
    let mut env = settings
        .remove("env")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    if let Some(ProviderInfo {
        settings: ProviderSettings::Claude(old_config),
        ..
    }) = old
    {
        for key in old_config.env_vars.keys() {
            env.remove(key);
        }
    }

    for (key, value) in &new_config.env_vars {
        env.insert(key.clone(), value.clone());
    }

    settings.insert("env".to_string(), Value::Object(env));
    info!(
        "provider switch: claude — writing env vars to {}",
        settings_path.display()
    );
    write_json_pretty(&settings_path, &Value::Object(settings))
        .map_err(|err| ArcError::new(format!("failed to write Claude provider config: {err}")))
}
