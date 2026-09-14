mod claude;
mod codex;
pub mod test;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use log::{info, warn};
use once_cell::sync::Lazy;
use toml::Table;

use crate::agent::{ProviderKind, agent_spec, agent_specs};
use crate::error::{ArcError, Result};
use crate::io::{read_toml_table, write_toml_pretty};
use crate::paths::ArcPaths;

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub agent: String,
    pub settings: ProviderSettings,
}

#[derive(Debug, Clone)]
pub enum ProviderSettings {
    Claude(ClaudeProviderConfig),
    Codex(CodexProviderConfig),
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeProviderConfig {
    pub env_vars: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub extra: Table,
}

const BASE_FIELDS: [&str; 4] = ["display_name", "description", "base_url", "api_key"];

pub(super) struct ProviderBaseConfig {
    display_name: String,
    description: String,
    base_url: Option<String>,
    api_key: Option<String>,
}

impl ProviderBaseConfig {
    fn parse(section: &Table, provider_name: &str) -> Result<Self> {
        let required_string = |field: &str| {
            section
                .get(field)
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    ArcError::new(format!(
                        "provider '{provider_name}' requires string field '{field}'"
                    ))
                })
        };
        let credential = |field: &str| -> Result<Option<String>> {
            if !section.contains_key(field) {
                return Ok(None);
            }
            let value = required_string(field)?;
            Ok((!value.is_empty()).then(|| value.to_string()))
        };
        let base_url = credential("base_url")?;
        let api_key = credential("api_key")?;
        match (&base_url, &api_key) {
            (Some(_), None) => {
                return Err(ArcError::new(format!(
                    "provider '{provider_name}': base_url requires api_key"
                )));
            }
            (None, Some(_)) => {
                return Err(ArcError::new(format!(
                    "provider '{provider_name}': api_key requires base_url"
                )));
            }
            _ => {}
        }
        Ok(Self {
            display_name: required_string("display_name")?.to_string(),
            description: required_string("description")?.to_string(),
            base_url,
            api_key,
        })
    }
}

#[derive(Clone, Copy)]
struct ProviderBackend {
    parse: fn(&ProviderBaseConfig, &Table) -> Result<ProviderSettings>,
    apply: fn(&ArcPaths, Option<&ProviderInfo>, &ProviderInfo) -> Result<()>,
}

static CLAUDE_PROVIDER_BACKEND: Lazy<ProviderBackend> = Lazy::new(|| ProviderBackend {
    parse: claude::parse_provider_config,
    apply: claude::apply_provider,
});

static CODEX_PROVIDER_BACKEND: Lazy<ProviderBackend> = Lazy::new(|| ProviderBackend {
    parse: codex::parse_provider_config,
    apply: codex::apply_provider,
});

pub fn supported_provider_agents() -> Vec<&'static str> {
    agent_specs()
        .iter()
        .filter(|spec| spec.provider_kind.is_some())
        .map(|spec| spec.id)
        .collect()
}

pub fn supports_provider_agent(agent: &str) -> bool {
    provider_backend(agent).is_some()
}

pub fn load_providers_for_agent(providers_dir: &Path, agent: &str) -> Result<Vec<ProviderInfo>> {
    let Some(backend) = provider_backend(agent) else {
        return Ok(Vec::new());
    };
    let path = providers_dir.join(format!("{agent}.toml"));
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(ArcError::new(format!(
                "failed to read {}: {err}",
                path.display()
            )));
        }
    };
    let table = toml::from_str::<Table>(&content)
        .map_err(|_| ArcError::new(format!("failed to parse {}", path.display())))?;

    table
        .iter()
        .map(|(section_name, section)| {
            let section = section.as_table().ok_or_else(|| {
                ArcError::new(format!("provider '{section_name}' must be a table"))
            })?;
            let base = ProviderBaseConfig::parse(section, section_name)?;
            let extra = section
                .iter()
                .filter(|(key, _)| !BASE_FIELDS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let settings = (backend.parse)(&base, &extra)?;
            Ok(ProviderInfo {
                name: section_name.to_string(),
                display_name: base.display_name,
                description: base.description,
                agent: agent.to_string(),
                settings,
            })
        })
        .collect()
}

pub fn read_active_provider(providers_dir: &Path, agent: &str) -> Option<String> {
    let path = providers_dir.join("active.toml");
    let content = fs::read_to_string(path).ok()?;
    let value = toml::from_str::<toml::Value>(&content).ok()?;
    value
        .get(agent)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("active"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

pub fn write_active_provider(
    providers_dir: &Path,
    agent: &str,
    provider_name: &str,
) -> std::io::Result<()> {
    let path = providers_dir.join("active.toml");
    let mut table = read_toml_table(&path);
    let mut agent_table = Table::new();
    agent_table.insert(
        "active".to_string(),
        toml::Value::String(provider_name.to_string()),
    );
    table.insert(agent.to_string(), toml::Value::Table(agent_table));
    write_toml_pretty(&path, &toml::Value::Table(table))
}

/// Apply a provider switch and record it as active — single atomic operation.
pub fn apply_provider(paths: &ArcPaths, provider: &ProviderInfo) -> Result<()> {
    info!(
        "provider switch: {} — {} → {}",
        provider.agent, provider.name, provider.display_name
    );
    crate::backup::backup_files(
        paths,
        "provider-use",
        &crate::backup::provider_backup_files(paths, &provider.agent),
    );
    let Some(backend) = provider_backend(&provider.agent) else {
        return Err(ArcError::new(format!(
            "unsupported agent '{}'",
            provider.agent
        )));
    };
    let providers_dir = paths.providers_dir();
    let old = match read_active_provider(&providers_dir, &provider.agent) {
        Some(name) => load_providers_for_agent(&providers_dir, &provider.agent)?
            .into_iter()
            .find(|p| p.name == name),
        None => None,
    };
    // Apply backend config first, then record active state.
    // If apply fails, active record stays unchanged — consistent state.
    (backend.apply)(paths, old.as_ref(), provider)?;
    write_active_provider(&providers_dir, &provider.agent, &provider.name)
        .map_err(|e| ArcError::new(format!("failed to record active provider: {e}")))
}

fn provider_backend(agent: &str) -> Option<&'static ProviderBackend> {
    let kind = agent_spec(agent)?.provider_kind?;
    match kind {
        ProviderKind::Claude => Some(&CLAUDE_PROVIDER_BACKEND),
        ProviderKind::Codex => Some(&CODEX_PROVIDER_BACKEND),
    }
}

/// Seed default "official" provider profile for each detected agent.
/// Only writes when the provider config file does not exist yet.
pub fn seed_default_providers(paths: &ArcPaths, cache: &crate::detect::DetectCache) {
    let providers_dir = paths.providers_dir();
    for spec in agent_specs()
        .iter()
        .filter(|spec| spec.provider_kind.is_some())
    {
        let config_path = providers_dir.join(format!("{}.toml", spec.id));
        if config_path.exists() {
            continue;
        }
        if cache.get_agent(spec.id).is_none() {
            continue;
        }
        if let Some(content) = spec.provider_seed
            && let Err(e) = crate::io::atomic_write_string(&config_path, content)
        {
            warn!("failed to seed provider config for {}: {e}", spec.id);
        }
    }
}
