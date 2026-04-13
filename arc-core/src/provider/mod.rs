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
    pub env_vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct CodexProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Clone, Copy)]
struct ProviderBackend {
    parse: fn(&Table) -> ProviderSettings,
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

pub fn load_providers_for_agent(providers_dir: &Path, agent: &str) -> Vec<ProviderInfo> {
    let Some(backend) = provider_backend(agent) else {
        return Vec::new();
    };
    let path = providers_dir.join(format!("{agent}.toml"));
    let Ok(content) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&content) else {
        warn!("failed to parse {}", path.display());
        return Vec::new();
    };
    let Some(table) = value.as_table() else {
        return Vec::new();
    };

    table
        .iter()
        .filter_map(|(section_name, section)| {
            let section = section.as_table()?;
            let display_name = section
                .get("display_name")
                .and_then(toml::Value::as_str)
                .unwrap_or(section_name)
                .to_string();
            let description = section
                .get("description")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(ProviderInfo {
                name: section_name.to_string(),
                display_name,
                description,
                agent: agent.to_string(),
                settings: (backend.parse)(section),
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
    let old = read_active_provider(&providers_dir, &provider.agent).and_then(|name| {
        load_providers_for_agent(&providers_dir, &provider.agent)
            .into_iter()
            .find(|p| p.name == name)
    });
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
