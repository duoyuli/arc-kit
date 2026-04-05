use log::info;
use serde_json::Value;

use crate::error::{ArcError, Result};
use crate::io::{read_json_map, write_json_pretty};
use crate::paths::ArcPaths;

use super::{ClaudeProviderConfig, ProviderInfo, ProviderSettings};

const METADATA_KEYS: &[&str] = &["display_name", "description"];

pub fn parse_provider_config(section: &toml::Table) -> ProviderSettings {
    let env_vars = section
        .iter()
        .filter(|(key, _)| !METADATA_KEYS.contains(&key.as_str()))
        .filter_map(|(key, value)| value.as_str().map(|v| (key.clone(), v.to_string())))
        .collect();
    ProviderSettings::Claude(ClaudeProviderConfig { env_vars })
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
        env.insert(key.clone(), Value::String(value.clone()));
    }

    settings.insert("env".to_string(), Value::Object(env));
    info!(
        "provider switch: claude — writing env vars to {}",
        settings_path.display()
    );
    write_json_pretty(&settings_path, &Value::Object(settings))
        .map_err(|err| ArcError::new(format!("failed to write Claude provider config: {err}")))
}
