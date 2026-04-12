use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use log::info;
use serde_json::{Map, Value};

use crate::error::{ArcError, Result};
use crate::io::{
    atomic_write_string, read_to_string_if_exists, read_toml_table, write_json_pretty,
    write_toml_pretty,
};
use crate::paths::ArcPaths;

use super::{CodexProviderConfig, ProviderInfo, ProviderSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexProviderMode {
    AuthOnly,
    Proxy,
}

#[derive(Debug)]
struct FileState {
    path: PathBuf,
    previous: Option<String>,
}

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
    let new_mode = provider_mode(config)?;
    let mut tracked_paths = vec![auth_path.clone(), config_path.clone()];
    if let Some(old) = old {
        tracked_paths.push(snapshot_path(paths, &old.name));
    }
    tracked_paths.push(snapshot_path(paths, &new.name));
    let previous_states = capture_file_states(&tracked_paths)?;

    let apply_result = (|| {
        if let Some(old) = old {
            snapshot_current_auth(paths, old, &auth_path)?;
        }

        let target_snapshot = resolve_target_auth_snapshot(paths, new)?;
        write_auth_config(paths, config, new_mode, target_snapshot.as_deref())?;
        write_main_config(paths, new, config)?;
        Ok(())
    })();

    if let Err(err) = apply_result {
        rollback_file_states(&previous_states)?;
        return Err(err);
    }

    Ok(())
}

fn write_auth_config(
    paths: &ArcPaths,
    config: &CodexProviderConfig,
    mode: CodexProviderMode,
    auth_snapshot: Option<&str>,
) -> Result<()> {
    let auth_path = paths.user_home().join(".codex").join("auth.json");
    match mode {
        CodexProviderMode::AuthOnly => match auth_snapshot {
            Some(auth_json) => restore_auth_snapshot(paths, auth_json),
            None => clear_auth_config(&auth_path),
        },
        CodexProviderMode::Proxy => {
            let api_key = config.api_key.as_deref().ok_or_else(|| {
                ArcError::new("failed to write Codex auth config: missing api_key".to_string())
            })?;
            let auth = Value::Object(Map::from_iter([(
                "OPENAI_API_KEY".to_string(),
                Value::String(api_key.to_string()),
            )]));
            write_json_pretty(&auth_path, &auth)
                .map_err(|err| ArcError::new(format!("failed to write Codex auth config: {err}")))
        }
    }
}

fn restore_auth_snapshot(paths: &ArcPaths, auth_json: &str) -> Result<()> {
    let Some(normalized) = normalized_auth_snapshot(auth_json, "restore")? else {
        let auth_path = paths.user_home().join(".codex").join("auth.json");
        return clear_auth_config(&auth_path);
    };
    let auth_path = paths.user_home().join(".codex").join("auth.json");
    atomic_write_string(&auth_path, &normalized)
        .map_err(|err| ArcError::new(format!("failed to restore Codex auth snapshot: {err}")))
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

fn provider_mode(config: &CodexProviderConfig) -> Result<CodexProviderMode> {
    match (config.base_url.as_deref(), config.api_key.as_deref()) {
        (None, None) => Ok(CodexProviderMode::AuthOnly),
        (Some(_), Some(_)) => Ok(CodexProviderMode::Proxy),
        (Some(_), None) => Err(ArcError::new(
            "invalid Codex provider: base_url requires api_key".to_string(),
        )),
        (None, Some(_)) => Err(ArcError::new(
            "invalid Codex provider: api_key requires base_url".to_string(),
        )),
    }
}

fn is_auth_only(config: &CodexProviderConfig) -> bool {
    config.api_key.is_none() && config.base_url.is_none()
}

fn snapshot_current_auth(paths: &ArcPaths, old: &ProviderInfo, auth_path: &Path) -> Result<()> {
    let ProviderSettings::Codex(config) = &old.settings else {
        return Ok(());
    };
    if !is_auth_only(config) {
        return Ok(());
    }

    let Some(auth_json) = read_to_string_if_exists(auth_path)
        .map_err(|err| ArcError::new(format!("failed to read Codex auth config: {err}")))?
    else {
        clear_snapshot_file(&snapshot_path(paths, &old.name))?;
        return Ok(());
    };

    let Some(normalized) = normalized_auth_snapshot(&auth_json, "snapshot")? else {
        clear_snapshot_file(&snapshot_path(paths, &old.name))?;
        return Ok(());
    };
    atomic_write_string(&snapshot_path(paths, &old.name), &normalized)
        .map_err(|err| ArcError::new(format!("failed to write Codex auth snapshot: {err}")))?;
    Ok(())
}

fn resolve_target_auth_snapshot(
    paths: &ArcPaths,
    provider: &ProviderInfo,
) -> Result<Option<String>> {
    let ProviderSettings::Codex(config) = &provider.settings else {
        return Ok(None);
    };
    if !is_auth_only(config) {
        return Ok(None);
    }

    let snapshot_path = snapshot_path(paths, &provider.name);
    if let Some(auth_json) = read_snapshot_file(&snapshot_path)? {
        return Ok(Some(auth_json));
    }

    Ok(None)
}

fn clear_auth_config(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|err| {
            ArcError::new(format!(
                "failed to clear Codex auth config {}: {err}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn clear_snapshot_file(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|err| {
            ArcError::new(format!(
                "failed to clear Codex auth snapshot {}: {err}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn snapshot_path(paths: &ArcPaths, provider_name: &str) -> PathBuf {
    paths
        .state_dir()
        .join("providers")
        .join("codex")
        .join(format!(
            "{}.auth.json",
            sanitize_provider_name(provider_name)
        ))
}

fn sanitize_provider_name(provider_name: &str) -> String {
    let mut sanitized = provider_name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = "provider".to_string();
    }
    if sanitized == provider_name {
        return sanitized;
    }
    let mut hasher = DefaultHasher::new();
    provider_name.hash(&mut hasher);
    format!("{sanitized}-{:016x}", hasher.finish())
}

fn read_snapshot_file(path: &Path) -> Result<Option<String>> {
    let Some(auth_json) = read_to_string_if_exists(path)
        .map_err(|err| ArcError::new(format!("failed to read Codex auth snapshot: {err}")))?
    else {
        return Ok(None);
    };
    let Some(normalized) = normalized_auth_snapshot(&auth_json, "restore")? else {
        clear_snapshot_file(path)?;
        return Ok(None);
    };
    if normalized != auth_json {
        atomic_write_string(path, &normalized).map_err(|err| {
            ArcError::new(format!("failed to normalize Codex auth snapshot: {err}"))
        })?;
    }
    Ok(Some(normalized))
}

fn normalized_auth_snapshot(auth_json: &str, action: &str) -> Result<Option<String>> {
    let mut auth = parse_auth_snapshot(auth_json, action)?;
    if matches!(auth.get("OPENAI_API_KEY"), Some(Value::Null)) {
        auth.remove("OPENAI_API_KEY");
    }
    if auth.is_empty() {
        return Ok(None);
    }
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(auth))
        .map_err(|err| ArcError::new(format!("failed to serialize Codex auth snapshot: {err}")))?;
    bytes.push(b'\n');
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|err| ArcError::new(format!("failed to serialize Codex auth snapshot: {err}")))
}

fn capture_file_states(paths: &[PathBuf]) -> Result<Vec<FileState>> {
    let mut unique_paths = paths.to_vec();
    unique_paths.sort();
    unique_paths.dedup();

    unique_paths
        .into_iter()
        .map(|path| {
            let previous = read_to_string_if_exists(&path).map_err(|err| {
                ArcError::new(format!("failed to read {}: {err}", path.display()))
            })?;
            Ok(FileState { path, previous })
        })
        .collect()
}

fn rollback_file_states(states: &[FileState]) -> Result<()> {
    for state in states.iter().rev() {
        rollback_file_state(&state.path, state.previous.as_deref())?;
    }
    Ok(())
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
