use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{ArcError, Result};
use crate::io::{atomic_write_string, read_to_string_if_exists};
use crate::market::git_url::parse_git_remote_parts;
use crate::models::MarketSource;
use crate::paths::ArcPaths;

pub const DEFAULT_BUILTIN_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/duoyuli/arc-kit/main/built-in/manifest.toml";
pub const BUILTIN_MANIFEST_URL_ENV: &str = "ARC_KIT_BUILTIN_MANIFEST_URL";

const EMBEDDED_BUILTIN_MANIFEST: &str = include_str!("../../../built-in/manifest.toml");
const EMBEDDED_MARKET_INDEX: &str = include_str!("../../../built-in/market/index.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinManifestDocument {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub index: BuiltinIndexSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuiltinIndexSection {
    #[serde(default)]
    pub market: BuiltinIndexEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinIndexEntry {
    #[serde(default = "default_market_index_path")]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIndexDocument {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, rename = "repo")]
    pub repos: Vec<MarketIndexRepo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIndexRepo {
    pub git_url: String,
    #[serde(default = "default_parser")]
    pub parser: String,
}

#[derive(Debug, Clone)]
pub struct MarketIndexStore {
    paths: ArcPaths,
}

impl Default for BuiltinManifestDocument {
    fn default() -> Self {
        Self {
            version: default_version(),
            index: BuiltinIndexSection::default(),
        }
    }
}

impl Default for BuiltinIndexEntry {
    fn default() -> Self {
        Self {
            path: default_market_index_path(),
        }
    }
}

impl Default for MarketIndexDocument {
    fn default() -> Self {
        Self {
            version: default_version(),
            updated_at: String::new(),
            repos: Vec::new(),
        }
    }
}

impl MarketIndexDocument {
    pub fn to_market_sources(&self) -> BTreeMap<String, MarketSource> {
        self.repos
            .iter()
            .map(|repo_entry| {
                let (owner, repo) = parse_git_remote_parts(&repo_entry.git_url).unwrap_or_default();
                let id = market_source_id(&repo_entry.git_url);
                (
                    id.clone(),
                    MarketSource {
                        id,
                        git_url: repo_entry.git_url.clone(),
                        parser: repo_entry.parser.clone(),
                        owner,
                        repo,
                        status: "indexed".to_string(),
                        last_updated_at: String::new(),
                        resource_count: 0,
                    },
                )
            })
            .collect()
    }
}

impl MarketIndexStore {
    pub fn new(paths: ArcPaths) -> Self {
        Self { paths }
    }

    pub fn load_effective(&self) -> MarketIndexDocument {
        match self.load_cached() {
            Ok(document) => document,
            _ => self.load_embedded(),
        }
    }

    pub fn load_cached(&self) -> Result<MarketIndexDocument> {
        let path = self.paths.market_index_cache();
        let Some(raw) =
            read_to_string_if_exists(&path).map_err(|err| ArcError::new(err.to_string()))?
        else {
            return Err(ArcError::new("market index cache not found"));
        };
        parse_market_index(&raw)
    }

    pub fn refresh(&self) -> Result<MarketIndexDocument> {
        self.refresh_from_manifest_url(&builtin_manifest_url())
    }

    pub fn refresh_from_manifest_url(&self, url: &str) -> Result<MarketIndexDocument> {
        let manifest_raw = fetch_text(url)?;
        let manifest = parse_builtin_manifest(&manifest_raw)?;
        let market_index_url = resolve_relative_location(url, &manifest.index.market.path)?;
        let market_raw = fetch_text(&market_index_url)?;
        let document = parse_market_index(&market_raw)?;
        self.write_cache(&market_raw)?;
        Ok(document)
    }

    fn write_cache(&self, raw: &str) -> Result<()> {
        let content = if raw.ends_with('\n') {
            raw.to_string()
        } else {
            format!("{raw}\n")
        };
        atomic_write_string(&self.paths.market_index_cache(), &content)
            .map_err(|err| ArcError::new(format!("failed to write market index cache: {err}")))
    }

    fn load_embedded(&self) -> MarketIndexDocument {
        let manifest = parse_builtin_manifest(EMBEDDED_BUILTIN_MANIFEST).unwrap_or_default();
        if manifest.index.market.path == default_market_index_path() {
            parse_market_index(EMBEDDED_MARKET_INDEX).unwrap_or_default()
        } else {
            MarketIndexDocument::default()
        }
    }
}

fn default_version() -> u32 {
    1
}

fn default_parser() -> String {
    "auto".to_string()
}

fn default_market_index_path() -> String {
    "market/index.toml".to_string()
}

fn builtin_manifest_url() -> String {
    std::env::var(BUILTIN_MANIFEST_URL_ENV)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_BUILTIN_MANIFEST_URL.to_string())
}

fn parse_builtin_manifest(raw: &str) -> Result<BuiltinManifestDocument> {
    toml::from_str(raw)
        .map_err(|err| ArcError::new(format!("failed to parse built-in manifest: {err}")))
}

fn parse_market_index(raw: &str) -> Result<MarketIndexDocument> {
    toml::from_str(raw).map_err(|err| ArcError::new(format!("failed to parse market index: {err}")))
}

fn fetch_text(url: &str) -> Result<String> {
    if let Some(path) = url.strip_prefix("file://") {
        return std::fs::read_to_string(path)
            .map_err(|err| ArcError::new(format!("failed to read built-in file: {err}")));
    }

    let response = ureq::get(url)
        .call()
        .map_err(|err| ArcError::new(format!("failed to download built-in file: {err}")))?;
    response
        .into_string()
        .map_err(|err| ArcError::new(format!("failed to read built-in response: {err}")))
}

fn resolve_relative_location(base: &str, relative: &str) -> Result<String> {
    if relative.starts_with("http://")
        || relative.starts_with("https://")
        || relative.starts_with("file://")
    {
        return Ok(relative.to_string());
    }

    if let Some(path) = base.strip_prefix("file://") {
        let base_path = std::path::Path::new(path);
        let Some(parent) = base_path.parent() else {
            return Err(ArcError::new(format!(
                "failed to resolve built-in path from manifest: {base}"
            )));
        };
        return Ok(format!("file://{}", parent.join(relative).display()));
    }

    let Some((prefix, _)) = base.rsplit_once('/') else {
        return Err(ArcError::new(format!(
            "failed to resolve built-in URL from manifest: {base}"
        )));
    };
    Ok(format!("{prefix}/{relative}"))
}

fn market_source_id(git_url: &str) -> String {
    super::git_url::slug_from_git_url(git_url)
}
