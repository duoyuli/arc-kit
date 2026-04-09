use log::info;
use serde_json::{Map, Value};

use crate::error::{ArcError, Result};
use crate::io::{
    atomic_write_string, read_json_map, read_to_string_if_exists, read_toml_table,
    write_json_pretty, write_toml_pretty,
};
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
        auth_json: section
            .get("auth_json")
            .and_then(toml::Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
    })
}

pub fn apply_provider(
    paths: &ArcPaths,
    old: Option<&ProviderInfo>,
    new: &ProviderInfo,
) -> Result<()> {
    let ProviderSettings::Codex(config) = &new.settings else {
        return Err(ArcError::new(format!(
            "provider '{}' does not have Codex settings",
            new.name
        )));
    };

    info!("provider switch: codex — applying '{}'", new.name);
    let auth_path = paths.user_home().join(".codex").join("auth.json");
    let config_path = paths.user_home().join(".codex").join("config.toml");
    let providers_path = paths.providers_dir().join("codex.toml");
    let auth_before = read_to_string_if_exists(&auth_path)
        .map_err(|err| ArcError::new(format!("failed to read Codex auth config: {err}")))?;
    let config_before = read_to_string_if_exists(&config_path)
        .map_err(|err| ArcError::new(format!("failed to read Codex config.toml: {err}")))?;
    let providers_before = read_to_string_if_exists(&providers_path)
        .map_err(|err| ArcError::new(format!("failed to read Codex provider registry: {err}")))?;
    let snapshot_update = prepare_old_auth_snapshot_update(paths, old)?;

    let apply_result = (|| {
        write_auth_config(paths, config)?;
        write_main_config(paths, new, config)?;
        if let Some(updated_registry) = &snapshot_update {
            atomic_write_string(&providers_path, updated_registry).map_err(|err| {
                ArcError::new(format!("failed to snapshot Codex auth config: {err}"))
            })?;
        }
        Ok(())
    })();

    if let Err(err) = apply_result {
        rollback_file_state(&auth_path, auth_before.as_deref())?;
        rollback_file_state(&config_path, config_before.as_deref())?;
        rollback_file_state(&providers_path, providers_before.as_deref())?;
        return Err(err);
    }

    Ok(())
}

fn write_auth_config(paths: &ArcPaths, config: &CodexProviderConfig) -> Result<()> {
    if let Some(auth_json) = &config.auth_json {
        if let Some(api_key) = &config.api_key {
            return restore_auth_snapshot_with_api_key(paths, auth_json, api_key);
        }
        return restore_auth_snapshot(paths, auth_json);
    }

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

fn restore_auth_snapshot(paths: &ArcPaths, auth_json: &str) -> Result<()> {
    parse_auth_snapshot(auth_json, "restore")?;
    let auth_path = paths.user_home().join(".codex").join("auth.json");
    atomic_write_string(&auth_path, auth_json)
        .map_err(|err| ArcError::new(format!("failed to restore Codex auth snapshot: {err}")))
}

fn restore_auth_snapshot_with_api_key(
    paths: &ArcPaths,
    auth_json: &str,
    api_key: &str,
) -> Result<()> {
    let mut auth = parse_auth_snapshot(auth_json, "restore")?;
    let current = auth.get("OPENAI_API_KEY").and_then(Value::as_str);
    if current != Some(api_key) {
        auth.insert(
            "OPENAI_API_KEY".to_string(),
            Value::String(api_key.to_string()),
        );
    }

    let auth_path = paths.user_home().join(".codex").join("auth.json");
    write_json_pretty(&auth_path, &Value::Object(auth)).map_err(|err| {
        ArcError::new(format!(
            "failed to restore Codex auth snapshot with api_key override: {err}"
        ))
    })
}

fn parse_auth_snapshot(auth_json: &str, action: &str) -> Result<Map<String, Value>> {
    let value = serde_json::from_str::<Value>(auth_json)
        .map_err(|err| ArcError::new(format!("failed to parse Codex auth snapshot: {err}")))?;
    value.as_object().cloned().ok_or_else(|| {
        ArcError::new(format!(
            "failed to {action} Codex auth snapshot: auth_json must be a JSON object"
        ))
    })
}

fn prepare_old_auth_snapshot_update(
    paths: &ArcPaths,
    old: Option<&ProviderInfo>,
) -> Result<Option<String>> {
    let Some(old) = old else {
        return Ok(None);
    };
    let ProviderSettings::Codex(config) = &old.settings else {
        return Ok(None);
    };
    if config.api_key.is_some() || config.base_url.is_some() {
        return Ok(None);
    }

    let auth_path = paths.user_home().join(".codex").join("auth.json");
    let Some(auth_json) = read_to_string_if_exists(&auth_path)
        .map_err(|err| ArcError::new(format!("failed to read Codex auth config: {err}")))?
    else {
        return Ok(None);
    };

    parse_auth_snapshot(&auth_json, "snapshot")?;

    let providers_path = paths.providers_dir().join("codex.toml");
    let mut providers = read_toml_table(&providers_path);
    let section = providers
        .get_mut(&old.name)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            ArcError::new(format!(
                "failed to snapshot Codex auth config: provider '{}' not found",
                old.name
            ))
        })?;
    section.insert("auth_json".to_string(), toml::Value::String(auth_json));
    toml::to_string_pretty(&toml::Value::Table(providers))
        .map(Some)
        .map_err(|err| ArcError::new(format!("failed to snapshot Codex auth config: {err}")))
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

fn rollback_file_state(path: &std::path::Path, previous: Option<&str>) -> Result<()> {
    match previous {
        Some(content) => atomic_write_string(path, content)
            .map_err(|err| ArcError::new(format!("failed to roll back {}: {err}", path.display()))),
        None => {
            if path.exists() {
                std::fs::remove_file(path).map_err(|err| {
                    ArcError::new(format!("failed to roll back {}: {err}", path.display()))
                })?;
            }
            Ok(())
        }
    }
}
