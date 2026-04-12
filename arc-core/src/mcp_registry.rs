//! Single-file MCP registry (`registry.toml`) plus built-in presets.

use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::capability::{validate_mcp_definition, McpDefinition};
use crate::error::{ArcError, Result};
use crate::paths::ArcPaths;

const REGISTRY_FILENAME: &str = "registry.toml";
const BUILTIN_PRESETS_TOML: &str = include_str!("../../built-in/mcp/index.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpRegistryFile {
    #[serde(default = "default_registry_version")]
    registry_version: u32,
    #[serde(default)]
    mcps: Vec<McpDefinition>,
}

fn default_registry_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpEntryOrigin {
    Builtin,
    User,
}

#[derive(Debug, Clone)]
pub struct McpCatalogEntry {
    pub definition: McpDefinition,
    pub origin: McpEntryOrigin,
}

fn load_builtin_presets() -> Result<Vec<McpDefinition>> {
    let parsed: McpRegistryFile = toml::from_str(BUILTIN_PRESETS_TOML)
        .map_err(|e| ArcError::new(format!("failed to parse built-in MCP presets: {e}")))?;
    let mut out = Vec::new();
    for mut m in parsed.mcps {
        validate_mcp_definition(&mut m)?;
        out.push(m);
    }
    Ok(out)
}

fn registry_path(paths: &ArcPaths) -> std::path::PathBuf {
    paths.mcps_dir().join(REGISTRY_FILENAME)
}

pub fn load_user_registry_mcps(paths: &ArcPaths) -> Result<Vec<McpDefinition>> {
    let path = registry_path(paths);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let body = fs::read_to_string(&path)
        .map_err(|e| ArcError::new(format!("failed to read {}: {e}", path.display())))?;
    let mut file: McpRegistryFile = toml::from_str(&body)
        .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))?;
    let mut out = Vec::new();
    for mut m in file.mcps.drain(..) {
        validate_mcp_definition(&mut m)?;
        out.push(m);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn merge_mcp_catalog(
    builtins: Vec<McpDefinition>,
    user: Vec<McpDefinition>,
) -> Vec<McpCatalogEntry> {
    let mut by_name: BTreeMap<String, McpCatalogEntry> = BTreeMap::new();
    for d in builtins {
        let name = d.name.clone();
        by_name.insert(
            name,
            McpCatalogEntry {
                definition: d,
                origin: McpEntryOrigin::Builtin,
            },
        );
    }
    for d in user {
        let name = d.name.clone();
        by_name.insert(
            name,
            McpCatalogEntry {
                definition: d,
                origin: McpEntryOrigin::User,
            },
        );
    }
    by_name.into_values().collect()
}

fn save_registry_file(paths: &ArcPaths, mcps: &[McpDefinition]) -> Result<()> {
    paths
        .ensure_arc_home()
        .map_err(|e| ArcError::new(format!("failed to ensure arc home: {e}")))?;
    let path = registry_path(paths);
    let file = McpRegistryFile {
        registry_version: 1,
        mcps: mcps.to_vec(),
    };
    let body = toml::to_string_pretty(&file)
        .map_err(|e| ArcError::new(format!("failed to serialize MCP registry: {e}")))?;
    fs::write(&path, body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

pub fn upsert_user_registry_mcp(paths: &ArcPaths, definition: &McpDefinition) -> Result<()> {
    let mut mcps = load_user_registry_mcps(paths)?;
    let pos = mcps.iter().position(|d| d.name == definition.name);
    if let Some(i) = pos {
        mcps[i] = definition.clone();
    } else {
        mcps.push(definition.clone());
    }
    mcps.sort_by(|a, b| a.name.cmp(&b.name));
    save_registry_file(paths, &mcps)
}

pub fn remove_user_registry_mcp(paths: &ArcPaths, name: &str) -> Result<bool> {
    let mut mcps = load_user_registry_mcps(paths)?;
    let before = mcps.len();
    mcps.retain(|d| d.name != name);
    if mcps.len() == before {
        return Ok(false);
    }
    save_registry_file(paths, &mcps)?;
    Ok(true)
}

pub fn builtin_mcp_definitions() -> Result<Vec<McpDefinition>> {
    load_builtin_presets()
}

pub fn load_merged_mcp_catalog(paths: &ArcPaths) -> Result<Vec<McpCatalogEntry>> {
    let builtins = load_builtin_presets()?;
    let user = load_user_registry_mcps(paths)?;
    Ok(merge_mcp_catalog(builtins, user))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::McpTransportType;

    #[test]
    fn builtin_presets_parse() {
        let v = load_builtin_presets().unwrap();
        assert!(v
            .iter()
            .any(|d| d.name == "filesystem" && d.transport == McpTransportType::Stdio));
        assert!(v
            .iter()
            .any(|d| d.name == "drawio" && d.transport == McpTransportType::Stdio));
        assert!(v.iter().any(|d| {
            d.name == "sequential-thinking" && d.transport == McpTransportType::Stdio
        }));
        assert!(v.iter().any(|d| {
            d.name == "zhipu-web-search" && d.transport == McpTransportType::StreamableHttp
        }));
    }

    #[test]
    fn merge_user_overrides_builtin() {
        let builtins = vec![McpDefinition {
            name: "filesystem".to_string(),
            targets: None,
            transport: McpTransportType::Stdio,
            command: Some("npx".to_string()),
            args: vec!["-y".to_string(), "pkg".to_string()],
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: None,
            headers: BTreeMap::new(),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: None,
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        }];
        let user = vec![McpDefinition {
            name: "filesystem".to_string(),
            targets: None,
            transport: McpTransportType::Stdio,
            command: Some("custom".to_string()),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
            env_file: None,
            url: None,
            headers: BTreeMap::new(),
            timeout: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled: None,
            required: None,
            trust: None,
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            oauth: None,
            description: None,
        }];
        let merged = merge_mcp_catalog(builtins, user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].definition.command.as_deref(), Some("custom"));
        assert_eq!(merged[0].origin, McpEntryOrigin::User);
    }
}
