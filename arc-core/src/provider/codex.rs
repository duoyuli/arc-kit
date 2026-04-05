use log::info;
use serde_json::Value;

use crate::error::{ArcError, Result};
use crate::io::{read_json_map, read_toml_table, write_json_pretty, write_toml_pretty};
use crate::paths::ArcPaths;

use super::{CodexProviderConfig, ProviderInfo, ProviderSettings};

pub fn parse_provider_config(section: &toml::Table) -> ProviderSettings {
    ProviderSettings::Codex(CodexProviderConfig {
        api_key: section
            .get("api_key")
            .and_then(toml::Value::as_str)
            .map(str::to_string),
        base_url: section
            .get("base_url")
            .and_then(toml::Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
    })
}

pub fn apply_provider(
    paths: &ArcPaths,
    _old: Option<&ProviderInfo>,
    new: &ProviderInfo,
) -> Result<()> {
    let ProviderSettings::Codex(config) = &new.settings else {
        return Err(ArcError::new(format!(
            "provider '{}' does not have Codex settings",
            new.name
        )));
    };

    info!("provider switch: codex — applying '{}'", new.name);
    write_auth_config(paths, config)?;
    write_main_config(paths, new, config)
}

fn write_auth_config(paths: &ArcPaths, config: &CodexProviderConfig) -> Result<()> {
    let auth_path = paths.user_home().join(".codex").join("auth.json");
    let mut auth = read_json_map(&auth_path);
    auth.insert(
        "OPENAI_API_KEY".to_string(),
        config
            .api_key
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    write_json_pretty(&auth_path, &Value::Object(auth))
        .map_err(|err| ArcError::new(format!("failed to write Codex auth config: {err}")))
}

fn write_main_config(
    paths: &ArcPaths,
    provider: &ProviderInfo,
    config: &CodexProviderConfig,
) -> Result<()> {
    let config_path = paths.user_home().join(".codex").join("config.toml");
    let mut config_table = read_toml_table(&config_path);

    if let Some(base_url) = &config.base_url {
        config_table.insert(
            "model_provider".to_string(),
            toml::Value::String(provider.name.clone()),
        );

        let mut provider_table = toml::Table::new();
        provider_table.insert(
            "name".to_string(),
            toml::Value::String(provider.display_name.clone()),
        );
        provider_table.insert(
            "base_url".to_string(),
            toml::Value::String(base_url.clone()),
        );

        let model_providers = config_table
            .entry("model_providers".to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let Some(model_providers) = model_providers.as_table_mut() else {
            return Err(ArcError::new(
                "failed to write Codex config.toml: model_providers is not a table",
            ));
        };
        model_providers.insert(provider.name.clone(), toml::Value::Table(provider_table));
    } else {
        config_table.remove("model_provider");
    }

    config_table.remove("openai_base_url");
    write_toml_pretty(&config_path, &toml::Value::Table(config_table))
        .map_err(|err| ArcError::new(format!("failed to write Codex config.toml: {err}")))
}
