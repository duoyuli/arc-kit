use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::capability::{SourceScope, SubagentDefinition, validate_subagent_definition};
use crate::error::{ArcError, Result};
use crate::paths::ArcPaths;

const BUILTIN_SUBAGENT_INDEX_TOML: &str = include_str!("../../built-in/subagent/index.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentEntryOrigin {
    Builtin,
    User,
}

#[derive(Debug, Clone)]
pub struct SubagentCatalogEntry {
    pub definition: SubagentDefinition,
    pub origin: SubagentEntryOrigin,
    pub prompt_body: String,
}

#[derive(Deserialize)]
struct BuiltinSubagentRegistryFile {
    #[serde(default)]
    subagents: Vec<BuiltinSubagentEntry>,
}

#[derive(Deserialize)]
struct BuiltinSubagentEntry {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    targets: Option<Vec<String>>,
    prompt: String,
}

fn builtin_cache_dir(paths: &ArcPaths) -> PathBuf {
    paths.builtin_cache_dir().join("subagent")
}

fn materialize_builtin_prompt(paths: &ArcPaths, name: &str, prompt_body: &str) -> Result<PathBuf> {
    let dir = builtin_cache_dir(paths);
    fs::create_dir_all(&dir)
        .map_err(|e| ArcError::new(format!("failed to create {}: {e}", dir.display())))?;
    let prompt_path = dir.join(format!("{name}.md"));
    fs::write(&prompt_path, prompt_body)
        .map_err(|e| ArcError::new(format!("failed to write {}: {e}", prompt_path.display())))?;
    Ok(prompt_path)
}

fn load_builtin_subagents(paths: &ArcPaths) -> Result<Vec<SubagentCatalogEntry>> {
    let parsed: BuiltinSubagentRegistryFile = toml::from_str(BUILTIN_SUBAGENT_INDEX_TOML)
        .map_err(|e| ArcError::new(format!("failed to parse built-in subagent index: {e}")))?;
    let mut entries = Vec::new();
    for item in parsed.subagents {
        let prompt_path = materialize_builtin_prompt(paths, &item.name, &item.prompt)?;
        let mut definition = SubagentDefinition {
            name: item.name,
            description: item.description,
            targets: item.targets,
            prompt_file: prompt_path.display().to_string(),
        };
        validate_subagent_definition(&mut definition, SourceScope::Global, paths.home())?;
        entries.push(SubagentCatalogEntry {
            definition,
            origin: SubagentEntryOrigin::Builtin,
            prompt_body: item.prompt,
        });
    }
    entries.sort_by(|a, b| a.definition.name.cmp(&b.definition.name));
    Ok(entries)
}

fn load_user_subagents(paths: &ArcPaths) -> Result<Vec<SubagentCatalogEntry>> {
    let mut entries = Vec::new();
    let dir = paths.subagents_dir();
    let Ok(items) = fs::read_dir(&dir) else {
        return Ok(entries);
    };
    for item in items.flatten() {
        let path = item.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let body = fs::read_to_string(&path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", path.display())))?;
        let mut definition: SubagentDefinition = toml::from_str(&body)
            .map_err(|e| ArcError::new(format!("failed to parse {}: {e}", path.display())))?;
        let prompt_path = paths
            .subagents_dir()
            .join(format!("{}.md", definition.name));
        let prompt_body = fs::read_to_string(&prompt_path)
            .map_err(|e| ArcError::new(format!("failed to read {}: {e}", prompt_path.display())))?;
        definition.prompt_file = prompt_path.display().to_string();
        validate_subagent_definition(&mut definition, SourceScope::Global, paths.home())?;
        entries.push(SubagentCatalogEntry {
            definition,
            origin: SubagentEntryOrigin::User,
            prompt_body,
        });
    }
    entries.sort_by(|a, b| a.definition.name.cmp(&b.definition.name));
    Ok(entries)
}

pub fn load_merged_subagent_catalog(paths: &ArcPaths) -> Result<Vec<SubagentCatalogEntry>> {
    let mut by_name: BTreeMap<String, SubagentCatalogEntry> = BTreeMap::new();
    for entry in load_builtin_subagents(paths)? {
        by_name.insert(entry.definition.name.clone(), entry);
    }
    for entry in load_user_subagents(paths)? {
        by_name.insert(entry.definition.name.clone(), entry);
    }
    Ok(by_name.into_values().collect())
}

pub fn find_global_subagent(paths: &ArcPaths, name: &str) -> Result<Option<SubagentCatalogEntry>> {
    Ok(load_merged_subagent_catalog(paths)?
        .into_iter()
        .find(|entry| entry.definition.name == name))
}
